use std::{
	path::PathBuf,
	process::{self, Command},
};

fn main() {
	println!("cargo::rerun-if-changed=shaders/shader.slang");

	let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("shader.spv");

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
		])
		.arg(out_path.as_os_str())
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
