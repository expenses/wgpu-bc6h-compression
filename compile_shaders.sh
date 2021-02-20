#!/bin/sh

glslc -fshader-stage=comp shaders/2d.comp.hlsl -o shaders/compiled/2d.comp.spv
glslc -DPUSH_CONSTANTS=1 -fshader-stage=comp shaders/2d.comp.hlsl -o shaders/compiled/2d_push_constants.comp.spv
glslc -fshader-stage=comp shaders/3d.comp.hlsl -o shaders/compiled/3d.comp.spv

spirv-opt shaders/compiled/2d.comp.spv -O -o shaders/compiled/2d.comp.spv
spirv-opt shaders/compiled/2d_push_constants.comp.spv -O -o shaders/compiled/2d_push_constants.comp.spv
spirv-opt shaders/compiled/3d.comp.spv -O -o shaders/compiled/3d.comp.spv
