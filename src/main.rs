mod render;

use render::TriangleApplication;

fn main() {
	let mut application = TriangleApplication::default();
	application.render();
}
