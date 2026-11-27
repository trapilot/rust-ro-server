# Movement Sync Issues During Combat

## 1. [HIGH] Mob Not Sent Stop Packet When Attacked
**File:** `server/src/server/service/map_instance_service.rs:145-174`

When `mob_being_attacked()` is called:
- `mob.last_attacked_at = tick` is set
- Server pauses mob movement during `damage_motion` (in `map_instance_loop.rs:134`)
- **No `PacketZcStopMove` sent to client**
- Movement queue is NOT cleared

**Result:** Client continues animating mob along original path while server has paused it. When mob resumes, positions diverge → visible teleport.

**Original Fix Proposal:** Send `PacketZcStopMove` with mob's current position when attacked, and clear movement queue.

### Research Findings (rathena)

**rathena does NOT send `PacketZcStopMove` when mobs are attacked.** Instead, it uses a damage motion delay mechanism:

- Sets `canmove_tick` to prevent movement for `dmotion` (damage motion) duration
- The mob AI simply doesn't issue new movement commands during hitlock/flinch time
- Movement queue is NOT explicitly cleared - mobs just don't process movement while `DIFF_TICK(tick, md->ud.canmove_tick) > 0`
- The `dmotion` value is stored in mob database (e.g., "damage motion = 550" milliseconds)
- Mobs with Endure status have `dMotion = 0` and don't flinch

**Correct Implementation:**
- Do NOT send `PacketZcStopMove` for mob damage
- Use `canmove_tick` delay approach instead
- Mob AI should check if enough time has passed before issuing new movement commands
- The client naturally shows the flinch animation based on the damage packet

**Sources:**
- [rathena mob.cpp](https://github.com/rathena/rathena/blob/master/src/map/mob.cpp)
- [rathena monster.conf](https://github.com/rathena/rathena/blob/master/conf/battle/monster.conf)
- [Ragnarok Research Lab - Movement](https://ragnarokresearchlab.github.io/game-mechanics/movement/)

---

## 2. Attack Tick Initialization Bug
**File:** `server/src/server/game_loop.rs:152`

`set_attack()` is called with `tick=0` instead of current tick. This causes `last_attack_tick=0` and `last_attack_motion=0`, making the movement delay check in `movement.rs:57` always evaluate to false until first attack completes.

**Impact:** Movement is never blocked during attack initiation phase.

### Research Findings (rathena)

**Should use CURRENT TICK, not 0.** rathena initializes attack timing to future tick values:

```c
attackabletime = gettick() + status_get_amotion(&bl) + delay
```

Key timing variables in rathena's `unit_data` structure:
- `attackabletime`: When the unit can next perform an attack (absolute tick value)
- `canact_tick`: When the unit can perform any action - set to current tick + delays
- `attacktimer`: Active attack action timer

Setting to 0 would make the unit immediately attackable since `DIFF_TICK(current_tick, 0) >= 0` is always true.

**Correct Implementation:**
- Pass current tick to `set_attack()`
- Calculate `last_attack_tick = current_tick + attack_motion`
- Use absolute future tick values, not relative offsets from 0

**Sources:**
- [rathena unit.cpp](https://github.com/rathena/rathena/blob/master/src/map/unit.cpp)
- [ASPD - iRO Wiki Classic](https://irowiki.org/classic/ASPD)

---

## 3. Race Condition: Attack Queue vs Movement Handler
**Files:** `game_loop.rs`, `request_handler/movement.rs`

- Attack events are queued and processed at 40ms tick rate (game loop)
- Movement requests are processed immediately in client thread
- Player can move before attack state is fully initialized

### Research Findings (rathena)

**rathena uses a queue-based approach with `stepaction` mechanism:**

- Movement uses `unit_walktoxy_timer()` for cell-by-cell progression
- Attack requests during movement are **queued** via `stepaction`
- The `stepaction` stores pending commands for execution at next cell boundary
- Timer validation (`ud->walktimer == tid`) prevents stale timer processing
- `ud->state.change_walk_target` indicates destination changes mid-movement

**Important distinction - Animation Delay vs Lock Delay:**
- **Animation Delay**: Movement IS ALLOWED, only attacks/skills blocked
- **Lock Delay** (invalid actions): Movement IS RESTRICTED

The client naturally blocks sending attack/skill commands during animation delay, so server-side handling is mainly a safety mechanism.

**Correct Implementation:**
- Movement takes priority - pending attacks wait for position validation
- Consider processing movement and attacks in same thread/loop
- The current multi-threaded design creates inherent race conditions that rathena avoids

**Sources:**
- [rathena unit.cpp](https://github.com/rathena/rathena/blob/master/src/map/unit.cpp)
- [Walk Delay Cleanup Fix](https://github.com/rathena/rathena/commit/13651d57e18730d42f55aaebe5e7eb68701b9895)

---

## 4. Position Mismatch on Movement Cancel
**File:** `server/src/server/service/character/character_service.rs:1530-1540`

`cancel_movement()` sends `PacketZcStopMove` with server's `character.x/y`. If movement loop (16ms tick) hasn't updated position yet, client sees teleport back.

### Research Findings (rathena)

**rathena uses `clif_fixpos()` strategically at key events:**

Position sync packets (`ZC_STOP_MOVE` 0x88) are sent:
- After taking damage (with walkdelay applied)
- After skill casting completion
- After knockback effects
- When status changes affect movement
- When walkpath is completed

The packet contains the **server's authoritative position**:
```c
struct PACKET_ZC_STOP_MOVE {
  short PacketType;  // 0x88
  unsigned long AID; // Account ID
  short xPos;        // Final X coordinate
  short yPos;        // Final Y coordinate
}
```

**Known issue:** Client uses hardcoded attack animation lengths from internal resources and ignores server `aMotion` values, causing timing mismatch windows.

**rathena workaround:** After damage, updates position every cell for 1000ms to handle Endure hits and desync cases.

**Correct Implementation:**
- This is somewhat expected behavior
- Call `clif_fixpos()` after damage events
- Consider tracking last damage time and updating position more frequently for a brief period
- The slight teleport on cancel is acceptable if positions are correct

**Sources:**
- [Position Sync Improvements](https://github.com/rathena/rathena/commit/ccbdda6b82efb4eaea6979506d85b349eeb6bb04)
- [Sync position after endure hit](https://github.com/rathena/rathena/commit/b7b6812fcf13275b54561c6a6327bb2338ad4eb7)

---

## 5. Tick Rate Mismatch
- Player movement loop: 16ms
- Game loop (attack processing): 40ms
- Mob movement loop: 20ms

Position updates don't align, causing inconsistent state during attack-move interactions.

### Research Findings (rathena)

**rathena uses a single-threaded main loop with 20ms base tick:**

- `TIMER_MIN_INTERVAL = 20ms` (changed from 50ms in 2014)
- All systems (movement, combat, skills, status effects) share the **same 20ms tick**
- Movement happens at 150ms intervals (multiples of 20ms base)
- The map-server is **fully single-threaded** - no parallel gameplay task processing
- This design prevents synchronization issues entirely

**Why 20ms:**
- Recommended for "perfect server-client syncing"
- Can be increased if server can't handle load, but moves away from recommended standard

**Correct Implementation:**
- Consider unifying tick rates to avoid sync issues
- rathena's approach: single 20ms loop handles everything
- Current multi-rate design (16/20/40ms) creates inherent synchronization challenges
- At minimum, ensure all rates are multiples of a common base

**Sources:**
- [rathena timer.cpp](https://github.com/rathena/rathena/blob/master/src/common/timer.cpp)
- [New default timer interval announcement](https://rathena.org/board/topic/99112-new-default-timer-interval-50ms-20ms/)
- [Movement Speed - iRO Wiki](https://irowiki.org/classic/Movement_Speed)

---

## 6. Movement Delay Calculation
**File:** `server/src/server/request_handler/movement.rs:57-58`

```rust
if attack.last_attack_tick + attack.last_attack_motion - 40 > start_at
```

Subtracts 40ms (one game tick), allowing movement 40ms before attack animation ends. Client may expect full animation lock.

### Research Findings (rathena)

**rathena formula is `AttackMotion + [200 - MaxASPD] × 10`**

- No explicit subtraction of a grace period
- The buffer **adds** time based on ASPD, doesn't subtract
- Higher ASPD (closer to 200) = shorter additional delay

**Walk delay types:**
- **Type 0 (damage-induced)**: Uses target's `dmotion` value, doesn't override higher delays
- **Type 1 (skill-induced)**: Can only increase delay, never decrease. Bosses can ignore skill delays but not damage delays

**Key formula components:**
- `battle_calc_walkdelay()` calculates walk delay
- Base is target's `dmotion` (damage motion)
- Multi-hit attacks add `multihit_delay * (hit_count - 1)` if > 10 hits
- Rate adjustment via `pc_walk_delay_rate` (players) or `walk_delay_rate` (NPCs)

**Important:** During animation delay, movement IS allowed - only skill/attack is blocked. The `-40` subtraction doesn't match rathena's approach.

**Correct Implementation:**
- Remove the `-40` subtraction
- Use proper ASPD-based formula: `AttackMotion + (200 - MaxASPD) * 10`
- Or simply use `AttackMotion` as the delay without arbitrary subtraction

**Sources:**
- [Walk Delay Cleanup Fix](https://github.com/rathena/rathena/commit/13651d57e18730d42f55aaebe5e7eb68701b9895)
- [Walkdelay Calculation Fix](https://github.com/rathena/rathena/commit/f59fd6d0b059a53d12bbfb588a7d1db03e8db261)
- [ASPD - iRO Wiki Classic](https://irowiki.org/classic/ASPD)

---

## Summary of Correct Fixes

| Issue | Original Proposal | Correct Fix (per rathena) |
|-------|-------------------|---------------------------|
| 1. Mob stop packet | Send `PacketZcStopMove` | Do NOT send - use `canmove_tick` delay |
<<<<<<< HEAD
| 2. Attack tick init | Pass current tick | Pass current tick + calculate future `attackabletime` |
=======
| 2. Attack tick init | ~~Pass current tick~~ | Keep 0 for immediate first attack. Use atomic `canmove_tick` for movement blocking |
>>>>>>> 19b53ebd (Rework movements wip)
| 3. Race condition | Process attack before movement | Queue-based `stepaction` system, single-threaded loop |
| 4. Position mismatch | Use interpolated position | Use server position, update frequently after damage |
| 5. Tick rate mismatch | Align tick rates | Single 20ms unified tick rate |
| 6. Movement delay calc | Remove -40 subtraction | Use `AttackMotion + (200 - MaxASPD) * 10` formula |

## Key Architectural Insight

rathena avoids most of these issues by using a **single-threaded 20ms game loop** that processes all events (movement, combat, skills) sequentially. The current multi-threaded, multi-rate design creates inherent synchronization challenges that require careful coordination.
