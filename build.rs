use std::process::{self, Command};

fn main() {
	println!("cargo::rerun-if-changed=shaders/shader.slang");

	if let Ok(status) = Command::new("slangc")
		.args([
			"shaders/shader.slang",
			"-target",
			"spirv",
			"-profile",
			"spirv_1_4",
			"-emit-spirv-directly",
			"-fvk-use-entrypoint-name",
			"-entry",
			"vertex_main",
			"-entry",
			"fragment_main",
			"-o",
			"shaders/shader.spv",
		])
		.spawn()
		.unwrap()
		.wait()
	{
		if !status.success() {
			process::exit(1)
		}
	} else {
		process::exit(1)
	}
}
