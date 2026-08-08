use std::path::PathBuf;

fn main() {
	// tell cargo to search this path for the shared library
	println!("cargo:rustc-link-search=/usr/lib/x86_64-linux-gnu");
	// tell cargo to tell rustc to link libglfw
	println!("cargo:rustc-link-lib=glfw");

	let bindings = bindgen::Builder::default()
		.header("wrapper.h")
		// glfw flags are i32
		.default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
		.generate()
		.unwrap();

	let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bindings.rs");
	bindings.write_to_file(out_path).unwrap();
}
