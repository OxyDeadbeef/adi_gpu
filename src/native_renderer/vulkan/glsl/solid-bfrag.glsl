// Aldaron's Device Interface / GPU
// Copyright (c) 2017 Plop Grizzly, Jeron Lau <jeron.lau@plopgrizzly.com>
// Licensed under the MIT LICENSE
//
// src/native_renderer/vulkan/glsl/solid.frag

#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (location = 0) in vec4 inColor;
layout (location = 0) out vec4 outColor;

void main() {
	outColor = inColor;
	if(outColor.a < 1.0) {
		discard;
	}
}
