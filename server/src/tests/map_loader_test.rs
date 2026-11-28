use std::collections::HashMap;
use std::path::Path;

use crate::server::boot::map_loader::MapLoader;
use crate::server::model::map_item::MapItems;

// load_maps takes a 'static dir and reads every file in it; a private copy keeps the test to one map.
fn dir_with_only(map_file: &str) -> &'static str {
    let dir = std::env::temp_dir().join(format!("map-loader-test-{}", map_file.replace('.', "-")));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/maps/pre-re").join(map_file);
    std::fs::copy(source, dir.join(map_file)).unwrap();
    Box::leak(dir.to_str().unwrap().to_string().into_boxed_str())
}

#[test]
fn load_maps_registers_the_tanthu_login_map() {
    let dir = dir_with_only("tanthu.mcache");

    let maps = MapLoader::load_maps(
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        &mut MapItems::default(),
        dir,
    );

    let tanthu = maps.get("tanthu").expect("tanthu must be loaded");
    assert_eq!(tanthu.x_size(), 64);
    assert_eq!(tanthu.length(), 64 * 64);
}
