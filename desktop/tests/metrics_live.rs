use stepx_companion::device::{format_summary, DeviceMetrics};

#[test]
fn phone_snapshot_reports_battery_health_voltage_resolution_and_wireless_address() {
    let metrics = DeviceMetrics::from_summary(&format_summary(
        "level: 100\nscale: 100\nstatus: 5\nhealth: 2\nvoltage: 4442\ntemperature: 291\n\
         Physical size: 1200x2670\n\
         24: wlan0    inet 198.51.100.27/24 brd 198.51.100.255 scope global wlan0\n",
    ));
    assert_eq!(metrics.battery_level, Some(100));
    assert_eq!(metrics.charging_state, "电量已满");
    assert_eq!(metrics.battery_health, "良好");
    assert_eq!(metrics.voltage, "4.44 V");
    assert_eq!(metrics.temperature, "29.1 °C");
    assert_eq!(metrics.screen_size, "1200 × 2670");
    assert_eq!(metrics.wifi_ipv4, "198.51.100.27");
}

#[test]
fn missing_snapshot_never_invents_battery_health_or_a_connection() {
    let metrics = DeviceMetrics::from_summary(&format_summary(""));
    assert_eq!(metrics.battery_level, None);
    assert_eq!(metrics.charging_state, "状态未检测");
    assert_eq!(metrics.battery_health, "未检测");
    assert_eq!(metrics.voltage, "未检测");
    assert_eq!(metrics.screen_size, "未检测");
    assert_eq!(metrics.wifi_ipv4, "未检测");
}

#[test]
fn snapshot_uses_effective_screen_size_and_scaled_battery_level() {
    let metrics = DeviceMetrics::from_summary(&format_summary(
        "level: 25\nscale: 50\nstatus: 4\nPhysical size: 1200x2670\nOverride size: 1080x2400\n",
    ));
    assert_eq!(metrics.battery_level, Some(50));
    assert_eq!(metrics.charging_state, "未充电");
    assert_eq!(metrics.screen_size, "1080 × 2400");
}

#[test]
fn storage_recognizes_userdata_bind_mounts_from_real_df_output() {
    let metrics = DeviceMetrics::from_summary(&format_summary(
        "/dev/block/dm-57 213G 28G 185G 14% /data/user/0\n/dev/fuse 213G 27G 186G 13% /storage/emulated\n",
    ));
    assert_eq!(metrics.storage_used, "28G");
    assert_eq!(metrics.storage_avail, "185G");
    assert_eq!(metrics.storage_percent, Some(14));
}
