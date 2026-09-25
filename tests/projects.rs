use solo::projects::{Catalog, MAX_PROJECTS, ProjectStore};
use std::fs;

#[test]
fn catalog_restores_project_names_roots_and_selection() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let store = ProjectStore::at_path(dir.path().join("config/projects.json"));
    let mut catalog = store.load().unwrap();
    let first = catalog.register(&a).unwrap();
    let second = catalog.register(&b).unwrap();
    catalog.rename(first, "設計🙂").unwrap();
    catalog.select(first).unwrap();
    store.save(&mut catalog).unwrap();
    let restored = store.load().unwrap();
    assert_eq!(restored, catalog);
    assert_eq!(restored.active, Some(first));
    assert_eq!(restored.get(first).unwrap().name, "設計🙂");
    assert_eq!(
        restored.get(second).unwrap().path,
        b.canonicalize().unwrap()
    );
}

#[test]
fn folders_are_deduplicated_but_same_names_in_different_roots_are_distinct() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a/service");
    let b = dir.path().join("b/service");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    let mut catalog = Catalog::default();
    let first = catalog.register(&a).unwrap();
    let second = catalog.register(&b).unwrap();
    assert_ne!(first, second);
    assert_eq!(catalog.register(&a.join(".")).unwrap(), first);
    assert_eq!(catalog.projects.len(), 2);
    assert_eq!(catalog.active, Some(first));
    #[cfg(unix)]
    {
        let link = dir.path().join("alias");
        std::os::unix::fs::symlink(&a, &link).unwrap();
        assert_eq!(catalog.register(&link).unwrap(), first);
        assert_eq!(catalog.projects.len(), 2);
    }
}

#[test]
fn removing_a_project_preserves_files_and_does_not_reuse_its_id() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(a.join("keep"), "user data").unwrap();
    let mut catalog = Catalog::default();
    let first = catalog.register(&a).unwrap();
    let second = catalog.register(&b).unwrap();
    catalog.remove(second).unwrap();
    assert_eq!(catalog.active, Some(first));
    catalog.remove(first).unwrap();
    assert!(catalog.active.is_none());
    assert!(catalog.projects.is_empty());
    assert_eq!(fs::read_to_string(a.join("keep")).unwrap(), "user data");
    assert!(catalog.register(&a).unwrap() > second);
}

#[test]
fn stale_writers_and_busy_store_cannot_overwrite_saved_projects() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::at_path(dir.path().join("projects.json"));
    let mut a = store.load().unwrap();
    let mut b = store.load().unwrap();
    a.register(dir.path()).unwrap();
    store.save(&mut a).unwrap();
    let before = fs::read(store.path()).unwrap();
    b.register(dir.path()).unwrap();
    assert!(store.save(&mut b).is_err());
    assert_eq!(b.revision, 0);
    assert_eq!(fs::read(store.path()).unwrap(), before);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(store.path().with_extension("json.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let revision = a.revision;
    assert!(store.save(&mut a).is_err());
    assert_eq!(a.revision, revision);
    assert_eq!(fs::read(store.path()).unwrap(), before);
    drop(lock);
    store.save(&mut a).unwrap();
    assert_eq!(
        a.revision,
        revision + 1,
        "a released lock file must not block later saves"
    );
}

#[test]
fn invalid_or_future_catalogs_are_never_silently_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::at_path(dir.path().join("projects.json"));
    for bad in [
        "{",
        r#"{"version":99,"revision":0,"next_id":1,"active":null,"projects":[]}"#,
    ] {
        fs::write(store.path(), bad).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(&mut Catalog::default()).is_err());
        assert_eq!(fs::read_to_string(store.path()).unwrap(), bad);
    }
}

#[test]
fn missing_folders_remain_registered_and_empty_selection_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("removed");
    fs::create_dir(&folder).unwrap();
    let store = ProjectStore::at_path(dir.path().join("projects.json"));
    let mut catalog = Catalog::default();
    let id = catalog.register(&folder).unwrap();
    store.save(&mut catalog).unwrap();
    fs::remove_dir(&folder).unwrap();
    assert_eq!(store.load().unwrap().projects.len(), 1);
    catalog.remove(id).unwrap();
    store.save(&mut catalog).unwrap();
    assert!(store.load().unwrap().projects.is_empty());
    assert_eq!(store.load().unwrap().active, None);
}

#[test]
fn invalid_registration_names_and_limits_preserve_existing_projects() {
    let dir = tempfile::tempdir().unwrap();
    let mut catalog = Catalog::default();
    let file = dir.path().join("file");
    fs::write(&file, "data").unwrap();
    assert!(catalog.register(&file).is_err());
    assert!(catalog.register(&dir.path().join("missing")).is_err());
    assert!(catalog.projects.is_empty());
    for index in 0..MAX_PROJECTS {
        let path = dir.path().join(index.to_string());
        fs::create_dir(&path).unwrap();
        catalog.register(&path).unwrap();
    }
    let original = catalog.clone();
    assert!(catalog.register(dir.path()).is_err());
    for name in ["", "   ", "a\nb"] {
        assert!(catalog.rename(1, name).is_err());
    }
    assert!(catalog.rename(1, &"x".repeat(65)).is_err());
    assert_eq!(catalog, original);
}
