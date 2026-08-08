struct VertexInput {
	float2 position;
	float3 color;
}

struct VertexOutput {
	float4 sv_position : SV_Position;
	float3 color;
};

struct UniformBuffer {
	float4x4 model;
	float4x4 view;
	float4x4 proj;
};
ConstantBuffer<UniformBuffer> uniform_buffer;

[shader("vertex")]
VertexOutput vertex_main(VertexInput input) {
	VertexOutput output;
	output.sv_position = mul(uniform_buffer.proj, mul(uniform_buffer.view, mul(uniform_buffer.model, float4(input.position, 0.0, 1.0))));
	output.color = input.color;
	return output;
}

[shader("fragment")]
float4 fragment_main(VertexOutput in_vertex) : SV_Target {
	return float4(in_vertex.color, 1.0);
}
