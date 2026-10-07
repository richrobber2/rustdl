#!/usr/bin/env python3
"""Extend an owned build copy of the pinned mobile backend; never edit its checkout."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--offline", "--locked", "--format-version", "1",
    "--filter-platform", "aarch64-linux-android", "--manifest-path", str(root / "native-ui/Cargo.toml"),
]))
package = next(item for item in metadata["packages"] if item["name"] == "gpui-pre-mobile")
assert "9075e3" in package["source"], "Unexpected mobile source revision"
original = pathlib.Path(package["manifest_path"]).parent
owned = root / "target/native-ui-platform"

def write_changed(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)

for folder in ("src", "shaders", "assets", "include"):
    source = original / folder
    if not source.is_dir():
        continue
    for file in source.rglob("*"):
        relative = file.relative_to(original)
        if any(part.startswith(".") for part in relative.parts) or not file.is_file():
            continue
        if str(relative) in ("src/lib.rs", "src/android/window.rs"):
            continue
        write_changed(owned / relative, file.read_bytes())

window = (original / "src/android/window.rs").read_text()
anchor = "impl PlatformWindow for AndroidPlatformWindow {"
assert window.count(anchor) == 1, "Pinned window adapter changed"
assert "fn a11y_init(" not in window, "Upstream now supplies accessibility; review bridge"
window = window.replace(anchor, anchor + "\n    fn a11y_init(&self, callbacks: gpui::A11yCallbacks) { crate::rustdl_accessibility::install(callbacks); }\n    fn a11y_tree_update(&self, tree: gpui::RustdlTreeUpdate) {crate::rustdl_accessibility::update(tree); }\n")
write_changed(owned / "src/android/window.rs", window.encode())
write_changed(owned / "src/lib.rs", (original / "src/lib.rs").read_bytes() + b"\n#[cfg(target_os = \"android\")]\npub mod rustdl_accessibility;\n")
write_changed(owned / "src/rustdl_accessibility.rs", (root / "native-ui/mobile-accessibility.rs").read_bytes())
core = next(item for item in metadata["packages"] if item["name"] == "gpui-pre")
core_original = pathlib.Path(core["manifest_path"]).parent
core_target = next(item for item in core["targets"] if item["name"] == "gpui")
core_lib = pathlib.Path(core_target["src_path"])
core_owned = root / "target/native-ui-core-platform"
write_changed(core_owned / "README.md", (core_original / "README.md").read_bytes())
for file in (core_original / "src").rglob("*"):
    if not file.is_file() or file == core_lib or file == core_original / "src/window.rs": continue
    relative = file.relative_to(core_original)
    if any(part.startswith(".") for part in relative.parts): continue
    write_changed(core_owned / relative, file.read_bytes())
core_relative = core_lib.relative_to(core_original)
write_changed(core_owned / "src/rustdl_a11y_action_policy.rs", (root / "native-ui/a11y-action-policy.rs").read_bytes())
core_window = (core_original / "src/window.rs").read_text()
replacements = [
    ("async_channel::unbounded::<accesskit::ActionRequest>();", "async_channel::bounded::<(u64, accesskit::ActionRequest)>(64);"),
    ("action_sender.send_blocking(request).log_err();", "action_sender.try_send((crate::rustdl_a11y_action_policy::generation(), request)).log_err();"),
    ("while let Ok(request) = action_receiver.recv().await {", "while let Ok((generation, request)) = action_receiver.recv().await {"),
    ("window.handle_a11y_action(request, cx);", "if crate::rustdl_a11y_action_policy::current(generation) { window.handle_a11y_action(request, cx); }"),
]
for original_text, replacement in replacements:
    assert core_window.count(original_text) == 1, "Pinned semantic action queue changed"
    core_window = core_window.replace(original_text, replacement, 1)
write_changed(core_owned / "src/window.rs", core_window.encode())
write_changed(core_owned / core_relative, core_lib.read_bytes() + b"\npub use accesskit::TreeUpdate as RustdlTreeUpdate;\npub mod rustdl_a11y_action_policy;\n")
write_changed(owned / "compile-map.json", json.dumps({
    "gpui_mobile": {"original": str(original / "src/lib.rs"), "owned": str(owned / "src/lib.rs")},
    "gpui": {"original": str(core_lib), "owned": str(core_owned / core_relative)},
}).encode())
