// Synthetic code, never compiled or executed.
fn unsafe_shell(input: &str) {
    // ruleid: rustdl.rust.formatted-shell-command
    Command::new("sh").arg("-c").arg(format!("echo {}", input));
    // ok: rustdl.rust.formatted-shell-command
    Command::new("echo").arg(input);
}
