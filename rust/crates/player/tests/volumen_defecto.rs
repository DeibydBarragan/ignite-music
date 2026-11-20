use nucleo::Config;
use player::{Cola, DEFAULT_VOLUME};
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn cola_nueva_usa_el_volumen_por_defecto_de_config() {
    let config = Config::from_map(PathBuf::from("."), &HashMap::new());
    assert_eq!(u32::from(DEFAULT_VOLUME), config.bot.default_volume);
    assert_eq!(Cola::nueva("guild-1").volumen, DEFAULT_VOLUME);
}
