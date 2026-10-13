use mouse_studio::{
    actions::{dispatcher, profile_script, shortcut},
    model::{ActionKind, Binding, Settings},
};

fn binding(kind: ActionKind, value: &str) -> Binding {
    Binding {
        kind,
        value: value.into(),
    }
}

#[test]
fn maps_all_action_kinds_and_validates_shortcuts() {
    assert_eq!(
        dispatcher(&binding(ActionKind::Application, "code")).unwrap(),
        Some("hl.dsp.exec_cmd(\"code\")".into())
    );
    assert_eq!(
        dispatcher(&binding(ActionKind::Workspace, "4")).unwrap(),
        Some("hl.dsp.focus({workspace=\"4\"})".into())
    );
    assert_eq!(
        shortcut("Ctrl+Shift+C").unwrap(),
        ("CTRL SHIFT".into(), "c".into())
    );
    assert_eq!(shortcut("ALT+Tab").unwrap(), ("ALT".into(), "Tab".into()));
    assert_eq!(shortcut("F12").unwrap(), ("".into(), "F12".into()));
    for value in [
        "CTRL+",
        "FAKE+C",
        "CTRL+CTRL+C",
        "CTRL+notakey",
        "CTRL+C; os.exit()",
    ] {
        assert!(shortcut(value).is_err(), "accepted {value}");
    }
    assert!(dispatcher(&binding(ActionKind::Workspace, "0")).is_err());
    assert!(dispatcher(&binding(ActionKind::Command, " ")).is_err());
    assert_eq!(dispatcher(&Binding::default()).unwrap(), None);
}

#[test]
fn hostile_command_text_stays_inside_lua_string_and_is_not_executed_by_save() {
    let value = "printf 'hello'; echo \"é\"\n]]); error('injected') --\\";
    let action = dispatcher(&binding(ActionKind::Command, value))
        .unwrap()
        .unwrap();
    assert!(action.contains("\\\"é\\\"\\n"));
    assert!(!action.contains('\n'));
    let mut settings = Settings::default();
    settings.active_mut().bindings[0] = binding(ActionKind::Command, value);
    let script = profile_script(settings.active()).unwrap();
    assert!(script.contains("device={inclusive=true,list={\"logitech-mx-anywhere-3s\"}}"));
    assert!(script.contains("set_enabled"));
    // Removing the owned focus listener is safe; removing a keybind is not.
    assert!(
        !script
            .replace("owner.focus_event:remove()", "")
            .contains(":remove(")
    );
    assert!(!script.contains("hl.unbind"));
    assert!(script.contains("function()"));
}

#[test]
fn function_keys_are_normalized() {
    assert_eq!(mouse_studio::actions::shortcut("CTRL+f01").unwrap().1, "F1");
}
