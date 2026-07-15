static float2 positions[3] = float2[](
	float2(0.0, -0.5),
	float2(0.5, 0.5),
	float2(-0.5, 0.5)
);

struct VertexOutput {
	float3 color;
	float4 sv_position : SV_Position;
};

[shader("vertex")]
VertexOutput vertex_main(uint vertex_id : SV_VertexID) {
	VertexOutput output;
	output.sv_position = float4(positions[vertex_id], 0.0, 1.0);
	return output;
}

[shader("fragment")]
float4 fragment_main() : SV_Target {
	return float4(1.0, 0.0, 0.0, 1.0);
}