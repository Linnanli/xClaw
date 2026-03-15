use desktop_client::skill_manager::SkillManager;

#[test]
fn test_skill_manager_creation() {
    let manager = SkillManager::new();
    let skills = manager.get_available_skills().unwrap();
    assert_eq!(skills.len(), 3);
}

#[test]
fn test_install_skill() {
    let mut manager = SkillManager::new();
    assert!(manager.install_skill("code-review".to_string()).is_ok());
    let installed = manager.get_installed_skills().unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].metadata.id, "code-review");
    assert!(installed[0].enabled);
}

#[test]
fn test_uninstall_skill() {
    let mut manager = SkillManager::new();
    manager.install_skill("code-review".to_string()).unwrap();
    assert!(manager.uninstall_skill("code-review".to_string()).is_ok());
    let installed = manager.get_installed_skills().unwrap();
    assert_eq!(installed.len(), 0);
}

#[test]
fn test_enable_disable_skill() {
    let mut manager = SkillManager::new();
    manager.install_skill("code-review".to_string()).unwrap();
    
    manager.disable_skill("code-review".to_string()).unwrap();
    let installed = manager.get_installed_skills().unwrap();
    assert!(!installed[0].enabled);

    manager.enable_skill("code-review".to_string()).unwrap();
    let installed = manager.get_installed_skills().unwrap();
    assert!(installed[0].enabled);
}

#[test]
fn test_install_nonexistent_skill() {
    let mut manager = SkillManager::new();
    let result = manager.install_skill("nonexistent".to_string());
    assert!(result.is_err());
}

#[test]
fn test_uninstall_nonexistent_skill() {
    let mut manager = SkillManager::new();
    let result = manager.uninstall_skill("nonexistent".to_string());
    assert!(result.is_err());
}

#[test]
fn test_multiple_skills_installation() {
    let mut manager = SkillManager::new();
    assert!(manager.install_skill("code-review".to_string()).is_ok());
    assert!(manager.install_skill("documentation".to_string()).is_ok());
    assert!(manager.install_skill("testing".to_string()).is_ok());
    
    let installed = manager.get_installed_skills().unwrap();
    assert_eq!(installed.len(), 3);
}
