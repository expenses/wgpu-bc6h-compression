#!/bin/sh

glslc -fshader-stage=comp shaders/shader.comp.hlsl -o shaders/compiled/2d.comp.spv
glslc -DCOMPRESS_3D=1 -fshader-stage=comp shaders/shader.comp.hlsl -o shaders/compiled/3d.comp.spv

glslc -DPUSH_CONSTANTS=1 -fshader-stage=comp shaders/shader.comp.hlsl -o shaders/compiled/2d_push_constants.comp.spv
glslc -DCOMPRESS_3D=1 -DPUSH_CONSTANTS=1 -fshader-stage=comp shaders/shader.comp.hlsl -o shaders/compiled/3d_push_constants.comp.spv

spirv-opt shaders/compiled/2d.comp.spv -O -o shaders/compiled/2d.comp.spv
spirv-opt shaders/compiled/3d.comp.spv -O -o shaders/compiled/3d.comp.spv

spirv-opt shaders/compiled/2d_push_constants.comp.spv -O -o shaders/compiled/2d_push_constants.comp.spv
spirv-opt shaders/compiled/3d_push_constants.comp.spv -O -o shaders/compiled/3d_push_constants.comp.spv
