fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("usage-hook") {
        let agent = args.next().unwrap_or_default();
        let mut hook_input = String::new();
        let mut stdin = std::io::stdin();
        let _ = std::io::Read::read_to_string(&mut stdin, &mut hook_input);
        let _ = skillbox_core::record_skill_usage_from_hook(
            &agent,
            &hook_input,
            skillbox_core::default_managed_root(),
        );
        return;
    }

    skillbox_desktop::run();
}
