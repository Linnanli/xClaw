use desktop_client::routine_manager::{RoutineManager, RoutineTrigger, RoutineAction};
use proptest::prelude::*;

prop_compose! {
    fn arb_routine_trigger()(trigger_type in 0..3u32) -> RoutineTrigger {
        match trigger_type {
            0 => RoutineTrigger::Manual,
            1 => RoutineTrigger::Time("0 9 * * *".to_string()),
            _ => RoutineTrigger::Event("test_event".to_string()),
        }
    }
}

prop_compose! {
    fn arb_routine_action()(
        action_type in "[a-z_]{5,15}",
    ) -> RoutineAction {
        RoutineAction {
            id: uuid::Uuid::new_v4().to_string(),
            action_type,
            parameters: serde_json::json!({}),
        }
    }
}

proptest! {
    #[test]
    fn prop_create_routine_succeeds(
        name in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let result = manager.create_routine(name, description, trigger, vec![]);
        prop_assert!(result.is_ok());
        let routine = result.unwrap();
        prop_assert_eq!(manager.get_all_routines().len(), 1);
        prop_assert_eq!(&manager.get_routine(&routine.id).unwrap().id, &routine.id);
    }

    #[test]
    fn prop_delete_routine_removes_it(
        name in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let routine = manager.create_routine(name, description, trigger, vec![]).unwrap();
        manager.delete_routine(&routine.id).unwrap();
        prop_assert_eq!(manager.get_all_routines().len(), 0);
    }

    #[test]
    fn prop_enable_disable_routine(
        name in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let routine = manager.create_routine(name, description, trigger, vec![]).unwrap();
        
        manager.disable_routine(&routine.id).unwrap();
        prop_assert_eq!(&manager.get_routine(&routine.id).unwrap().status, &desktop_client::routine_manager::RoutineStatus::Disabled);
        
        manager.enable_routine(&routine.id).unwrap();
        prop_assert_eq!(&manager.get_routine(&routine.id).unwrap().status, &desktop_client::routine_manager::RoutineStatus::Active);
    }

    #[test]
    fn prop_pause_routine(
        name in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let routine = manager.create_routine(name, description, trigger, vec![]).unwrap();
        
        manager.pause_routine(&routine.id).unwrap();
        prop_assert_eq!(&manager.get_routine(&routine.id).unwrap().status, &desktop_client::routine_manager::RoutineStatus::Paused);
    }

    #[test]
    fn prop_trigger_routine_creates_run(
        name in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let routine = manager.create_routine(name, description, trigger, vec![]).unwrap();
        
        let run = manager.trigger_routine(&routine.id).unwrap();
        prop_assert_eq!(run.status, "running");
        prop_assert_eq!(run.routine_id, routine.id);
    }

    #[test]
    fn prop_disabled_routine_cannot_trigger(
        name in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let routine = manager.create_routine(name, description, trigger, vec![]).unwrap();
        
        manager.disable_routine(&routine.id).unwrap();
        let result = manager.trigger_routine(&routine.id);
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_complete_routine_run(
        name in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let routine = manager.create_routine(name, description, trigger, vec![]).unwrap();
        
        let run = manager.trigger_routine(&routine.id).unwrap();
        manager.complete_routine_run(&run.id, true, None).unwrap();
        
        let runs = manager.get_routine_runs(&routine.id, 10);
        prop_assert_eq!(&runs[0].status, "success");
        prop_assert!(runs[0].completed_at.is_some());
    }

    #[test]
    fn prop_get_active_routines_filters(
        name1 in "[A-Z][a-z]{3,10}",
        name2 in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        let routine1 = manager.create_routine(name1, description.clone(), trigger.clone(), vec![]).unwrap();
        let routine2 = manager.create_routine(name2, description, trigger, vec![]).unwrap();
        
        manager.disable_routine(&routine1.id).unwrap();
        
        let active = manager.get_active_routines();
        prop_assert_eq!(active.len(), 1);
        prop_assert_eq!(&active[0].id, &routine2.id);
    }

    #[test]
    fn prop_multiple_routines_managed(
        name1 in "[A-Z][a-z]{3,10}",
        name2 in "[A-Z][a-z]{3,10}",
        name3 in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        trigger in arb_routine_trigger(),
    ) {
        let mut manager = RoutineManager::new();
        manager.create_routine(name1, description.clone(), trigger.clone(), vec![]).unwrap();
        manager.create_routine(name2, description.clone(), trigger.clone(), vec![]).unwrap();
        manager.create_routine(name3, description, trigger, vec![]).unwrap();
        
        prop_assert_eq!(manager.get_all_routines().len(), 3);
    }
}
