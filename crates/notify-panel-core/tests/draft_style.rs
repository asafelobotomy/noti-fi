//! Editing the "time left" indicator (timerStyle) on rules, zones, apps and the theme.

use notify_panel_core::draft::Draft;

const SAMPLE: &str = r#"{"zones": {"a": {"anchor": "top-left"}}, "rules": [{"name": "r", "match": {"appName": "x"}}]}"#;

#[test]
fn the_time_left_indicator_is_a_style_key_everywhere_a_style_can_go() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    d.set_rule_style(0, "timerStyle", "line,circle").unwrap();
    assert_eq!(d.value()["rules"][0]["timerStyle"], "line,circle");
    assert_eq!(d.rules()[0].style["timerStyle"], "line,circle");
    d.set_zone_style("a", "timerStyle", "frame").unwrap();
    assert_eq!(d.settings_view()["zones"][0]["style"]["timerStyle"], "frame");
    d.set_setting("theme.timerStyle", "circle").unwrap();
    assert_eq!(d.value()["theme"]["timerStyle"], "circle");
    assert_eq!(d.settings_view()["values"]["theme.timerStyle"], "circle");
    d.set_routing("Steam", &[("appName", "Steam")], "timerStyle", "none").unwrap();
    assert_eq!(d.routing_for(&[("appName", "Steam")]).style["timerStyle"], "none", "an explicit none is kept: it turns an inherited indicator off");
    d.set_rule_style(0, "timerStyle", "").unwrap();
    assert!(d.value()["rules"][0].get("timerStyle").is_none(), "empty text removes it (inherit)");
}
