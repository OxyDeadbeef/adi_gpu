// Aldaron's Device Interface / GPU
// Copyright (c) 2017 Plop Grizzly, Jeron Lau <jeron.lau@plopgrizzly.com>
// Licensed under the MIT LICENSE
//
// src/native_renderer/vulkan/glsl/faded.vert

#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (binding = 0) uniform UniformBuffer {
	mat4 matrix_transform;
	float alpha;
} uniforms;

layout (location = 0) in vec4 pos;
layout (location = 1) in vec4 texpos;

layout (location = 0) out vec4 texcoord;

void main() {
	texcoord = vec4(texpos.xyz, texpos.w * uniforms.alpha);
	gl_Position = uniforms.matrix_transform * pos;
}
