use terrain_hands::CameraModel;

#[test]
fn a_saved_camera_loads_back_and_a_bad_one_is_an_error() {
    let dir = std::env::temp_dir().join(format!("terrain-saved-camera-{}", std::process::id()));
    let path = dir.join("nested").join("calibration.json");
    assert_eq!(CameraModel::load(&path).unwrap(), None, "no file is no calibration");

    let camera = CameraModel {
        hfov_deg: 70.0,
        ..CameraModel::default()
    };
    camera.save(&path).unwrap();
    assert_eq!(CameraModel::load(&path).unwrap(), Some(camera));

    let implausible = CameraModel {
        hfov_deg: 170.0,
        ..camera
    };
    implausible.save(&path).unwrap();
    assert!(CameraModel::load(&path).is_err());
    std::fs::write(&path, "not json").unwrap();
    assert!(CameraModel::load(&path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
