mod render;

use render::TriangleApplication;

fn main() {
	let application = TriangleApplication::default();
	application.render();
}
