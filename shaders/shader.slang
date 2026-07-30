struct VertexInput {
	float2 position;
	float3 color;
}

struct VertexOutput {
	float3 color;
	float4 sv_position : SV_Position;
};

[shader("vertex")]
VertexOutput vertex_main(VertexInput input) {
	VertexOutput output;
	output.color = input.color;
	output.sv_position = float4(input.position, 0.0, 1.0);
	return output;
}

[shader("fragment")]
float4 fragment_main(VertexOutput in_vertex) : SV_Target {
	return float4(in_vertex.color, 1.0);
}
