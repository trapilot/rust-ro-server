# Monster Attacking Player - Implementation Plan

## Overview

Implement monster AI behavior for attacking players based on mode flags (aggressive, passive, assist/supportive). This uses the existing `MobAction` state machine which already defines `Chasing` and `Attacking` states but lacks implementation.

## Key Files to Modify

| File | Purpose |
|------|---------|
| `lib/models/src/enums/mob.rs` | Add `MobMode` bitflags |
| `server/src/server/state/mob.rs` | Extend Mob struct, add state transitions |
| `server/src/repository/model/mob_model.rs` | Load mode from database |
| `server/src/server/service/mob_service.rs` | Implement AI decision logic |
| `server/src/server/model/events/map_event.rs` | Add `MobAttackCharacter` event |
| `server/src/server/service/map_instance_service.rs` | Integrate AI, handle attacks |
| `server/src/server/map_instance_loop.rs` | Process mob attack events |
| `server/src/server/game_loop.rs` | Implement `CharacterDamage` handler |

---

## Phase 1: Data Structures

### 1.1 Add MobMode Bitflags

**File:** `lib/models/src/enums/mob.rs`

```rust
use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct MobMode: u32 {
        const CANMOVE           = 0x0000001;
        const LOOTER            = 0x0000002;
        const AGGRESSIVE        = 0x0000004;
        const ASSIST            = 0x0000008;
        const CASTSENSOR_IDLE   = 0x0000010;
        const NORANDOMWALK      = 0x0000020;
        const NOCAST            = 0x0000040;
        const CANATTACK         = 0x0000080;
        const CASTSENSOR_CHASE  = 0x0000200;
        const CHANGECHASE       = 0x0000400;
        const ANGRY             = 0x0000800;
        const CHANGETARGET_MELEE = 0x0001000;
        const CHANGETARGET_CHASE = 0x0002000;
        const TARGETWEAK        = 0x0004000;
        const RANDOMTARGET      = 0x0008000;
        const DETECTOR          = 0x2000000;
    }
}

impl Default for MobMode {
    fn default() -> Self {
        // Default passive mob: can move, can attack
        MobMode::CANMOVE | MobMode::CANATTACK
    }
}
```

### 1.2 Extend Mob Struct

**File:** `server/src/server/state/mob.rs`

Add fields to `Mob`:
- `mode: MobMode` - AI behavior flags
- `attack_range: u16` - range1 from MobModel
- `chase_range: u16` - range3 from MobModel
- `atk_delay: u32` - attack cooldown in ms
- `atk_motion: u32` - attack animation duration
- `atk1: u16, atk2: u16` - min/max attack damage

Add to `MobTiming`:
- `canattack_tick: AtomicU64` - next attack time

### 1.3 Add State Machine Transitions

**File:** `server/src/server/state/mob.rs`

```rust
impl Mob {
    // Transition to Chasing from Idle/Moving
    pub fn transition_to_chasing(&mut self, target_id: u32) -> bool;

    // Transition to Attacking from Idle/Chasing
    pub fn transition_to_attacking(&mut self, target_id: u32, tick: u128) -> bool;

    // Check attack cooldown
    pub fn can_attack(&self, tick: u128) -> bool;

    // Get current target from Chasing/Attacking state
    pub fn get_target_id(&self) -> Option<u32>;

    // Lose target - return to Idle
    pub fn lose_target(&mut self);

    // Modified flinching to store attacker for passive mobs
    pub fn transition_to_flinching_with_attacker(&mut self, attacker_id: u32, tick: u128);
}
```

---

## Phase 2: Database Loading

### 2.1 Load Mode Field

**File:** `server/src/repository/model/mob_model.rs`

Change line 139 from:
```rust
// model.set_mode(row.get::<i32,_>("element_level")); TODO: collect all modes
```
To:
```rust
model.set_mode(row.try_get::<i16, _>("mode").unwrap_or(0x81)); // Default: CANMOVE | CANATTACK
```

### 2.2 Update Mob Spawning

**File:** `server/src/server/service/map_instance_service.rs`

Pass mode and ranges to `Mob::new()` during spawn.

---

## Phase 3: Map Events

### 3.1 Add MobAttackCharacter Event

**File:** `server/src/server/model/events/map_event.rs`

```rust
pub enum MapEvent {
    // ... existing ...
    MobAttackCharacter(MobAttackCharacter),
}

#[derive(Debug, PartialEq, Clone)]
pub struct MobAttackCharacter {
    pub mob_id: u32,
    pub target_char_id: u32,
    pub damage: u32,
    pub attack_motion: u32,
    pub mob_x: u16,
    pub mob_y: u16,
}
```

---

## Phase 4: AI Implementation

### 4.1 Extend MobService

**File:** `server/src/server/service/mob_service.rs`

```rust
pub enum MobAIAction {
    Move(MobMovement),
    Attack(MobAttackCharacter),
}

impl MobService {
    // Main AI entry point - replaces simple action_move for AI mobs
    pub fn action_ai(&self, mob: &mut Mob, characters: &[MapItemSnapshot],
                     cells: &[u16], x_size: u16, y_size: u16, tick: u128) -> Option<MobAIAction>;

    // AI state handlers
    fn ai_idle(&self, ...) -> Option<MobAIAction>;     // Check aggro, random move
    fn ai_chasing(&self, ...) -> Option<MobAIAction>;  // Update path, check range
    fn ai_attacking(&self, ...) -> Option<MobAIAction>; // Execute attack

    // Helpers
    fn find_nearest_target(&self, mob: &Mob, characters: &[MapItemSnapshot]) -> Option<&MapItemSnapshot>;
    fn calculate_mob_attack_damage(&self, mob: &Mob) -> u32;
}
```

### AI Decision Flow

```
ai_idle():
  1. If passive mob has target_id (was attacked) -> chase/attack
  2. If AGGRESSIVE mode -> find nearest player in chase_range
  3. If target in attack_range -> transition to Attacking
  4. If target in chase_range -> transition to Chasing
  5. Otherwise -> random move (existing logic)

ai_chasing():
  1. Find target position
  2. If target left map -> lose_target()
  3. If target in attack_range -> transition to Attacking
  4. If target escaped chase_range -> lose_target()
  5. If not moving -> pathfind toward target

ai_attacking():
  1. Find target position
  2. If target left map -> lose_target()
  3. If target out of attack_range -> transition to Chasing
  4. Check attack cooldown (atk_delay)
  5. Execute attack -> return MobAIAction::Attack
```

---

## Phase 5: Integration

### 5.1 Update Map Instance Service

**File:** `server/src/server/service/map_instance_service.rs`

```rust
// Replace mobs_action with AI-aware version
pub fn mobs_ai_action(&self, map_instance_state: &mut MapInstanceState,
                      characters: &[MapItemSnapshot], tick: u128);

// Handle mob attack execution
pub fn mob_attack_character(&self, map_instance_state: &MapInstanceState,
                            attack: MobAttackCharacter, server_task_queue: &TasksQueue<GameEvent>, tick: u128);
```

Attack handling:
1. Send `PacketZcNotifyActShortDamage` with attack animation to area
2. Queue `GameEvent::CharacterDamage` to game loop

### 5.2 Update Map Instance Loop

**File:** `server/src/server/map_instance_loop.rs`

Add event handler:
```rust
MapEvent::MobAttackCharacter(attack) => {
    map_instance_service.mob_attack_character(...);
}
```

### 5.3 Implement CharacterDamage Handler

**File:** `server/src/server/game_loop.rs`

Replace placeholder at line 305-306:
```rust
GameEvent::CharacterDamage(damage) => {
    if let Some(character) = server_state_mut.characters_mut().get_mut(&damage.target_id) {
        let current_hp = character.status.hp;
        if damage.damage >= current_hp {
            character.status.hp = 0;
            // Handle death - transition to Dead state
        } else {
            character.status.hp = current_hp - damage.damage;
        }
        // Send HP update packet
    }
}
```

---

## Phase 6: Assist/Supportive Behavior

### 6.1 Modify mob_being_attacked

**File:** `server/src/server/service/map_instance_service.rs`

When a mob with `ASSIST` mode is attacked:
1. Store attacker_id as target (for passive response)
2. Find nearby mobs of same `mob_id` within their chase_range
3. Set their target_id to the attacker

---

## Implementation Order

1. **Phase 1.1**: Add `MobMode` bitflags to `lib/models/src/enums/mob.rs`
2. **Phase 1.2-1.3**: Extend `Mob` struct and add transitions in `server/src/server/state/mob.rs`
3. **Phase 2**: Load mode from database
4. **Phase 3**: Add `MobAttackCharacter` map event
5. **Phase 4**: Implement AI logic in `mob_service.rs`
6. **Phase 5.1-5.2**: Integrate AI into map instance service and loop
7. **Phase 5.3**: Implement `CharacterDamage` handler in game loop
8. **Phase 6**: Add assist behavior

---

## Verification

1. **Unit test**: AI state transitions
2. **Unit test**: Target selection logic
3. **Integration test**: Spawn aggressive mob, verify it attacks player
4. **Integration test**: Spawn passive mob, attack it, verify it retaliates
5. **Integration test**: Spawn assist mobs, attack one, verify others join
6. **Manual test**: Run server, observe mob behavior in-game

---

## Thread Safety Notes

- Mob state modifications happen in map instance thread (exclusive access)
- Character damage queued to game loop via `GameEvent::CharacterDamage`
- Attack timing uses `AtomicU64` in `MobTiming` (same pattern as `canmove_tick`)
- Character snapshots passed via `MapEvent::UpdateMobsFov` (already implemented)

---

## AI Flow Diagram

```
                     ┌─────────────────────────────────────────────────────────┐
                     │                   Map Instance Loop                      │
                     │                    (every 40ms)                          │
                     └─────────────────────┬───────────────────────────────────┘
                                           │
                                           ▼
                     ┌─────────────────────────────────────────────────────────┐
                     │              For each mob: action_ai()                   │
                     └─────────────────────┬───────────────────────────────────┘
                                           │
           ┌───────────────────────────────┼───────────────────────────────────┐
           │                               │                                    │
           ▼                               ▼                                    ▼
   ┌───────────────┐              ┌───────────────┐                   ┌───────────────┐
   │   ai_idle()   │              │ ai_chasing()  │                   │ ai_attacking()│
   │               │              │               │                   │               │
   │ • Check if    │              │ • Find target │                   │ • Check range │
   │   attacked    │              │ • Check range │                   │ • Check delay │
   │ • Aggressive? │              │ • Update path │                   │ • Execute atk │
   │   Find target │              │ • Lost target?│                   │               │
   │ • Random move │              │               │                   │               │
   └───────┬───────┘              └───────┬───────┘                   └───────┬───────┘
           │                               │                                    │
           └───────────────────────────────┼────────────────────────────────────┘
                                           │
                                           ▼
                               ┌───────────────────────┐
                               │   Return MobAIAction  │
                               │   • Move              │
                               │   • Attack            │
                               └───────────┬───────────┘
                                           │
                                           ▼
                               ┌───────────────────────┐
                               │   Process actions:    │
                               │   • Send packets      │
                               │   • Queue events      │
                               └───────────────────────┘
```
