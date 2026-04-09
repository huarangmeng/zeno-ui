pub const SHADERS: &str = r#"
    #include <metal_stdlib>
    using namespace metal;

    struct ColorVertex {
        float2 clip_position;
        float2 local_position;
        float2 size;
        float4 color;
        float radius;
    };

    struct ColorOut {
        float4 position [[position]];
        float2 local_position;
        float2 size;
        float4 color;
        float radius;
    };

    vertex ColorOut color_vertex(uint vid [[vertex_id]], const device ColorVertex* vertices [[buffer(0)]]) {
        ColorVertex v = vertices[vid];
        ColorOut out;
        out.position = float4(v.clip_position, 0.0, 1.0);
        out.local_position = v.local_position;
        out.size = v.size;
        out.color = v.color;
        out.radius = v.radius;
        return out;
    }

    fragment float4 color_fragment(ColorOut in [[stage_in]]) {
        float radius = min(in.radius, min(in.size.x, in.size.y) * 0.5);
        if (radius > 0.0) {
            float2 nearest = clamp(in.local_position, float2(radius, radius), in.size - float2(radius, radius));
            float2 delta = in.local_position - nearest;
            if (dot(delta, delta) > radius * radius) {
                discard_fragment();
            }
        }
        return in.color;
    }

    struct TextVertex {
        float2 clip_position;
        float2 uv;
        float4 color;
    };

    struct TextOut {
        float4 position [[position]];
        float2 uv;
        float4 color;
    };

    vertex TextOut text_vertex(uint vid [[vertex_id]], const device TextVertex* vertices [[buffer(0)]]) {
        TextVertex v = vertices[vid];
        TextOut out;
        out.position = float4(v.clip_position, 0.0, 1.0);
        out.uv = v.uv;
        out.color = v.color;
        return out;
    }

    constexpr sampler text_sampler(address::clamp_to_edge, filter::linear);

    fragment float4 text_fragment(TextOut in [[stage_in]], texture2d<float> mask [[texture(0)]]) {
        float alpha = mask.sample(text_sampler, in.uv).r;
        return float4(in.color.rgb, in.color.a * alpha);
    }

    struct CompositeVertex {
        float2 clip_position;
        float2 uv;
        float4 color;
    };

    struct CompositeOut {
        float4 position [[position]];
        float2 uv;
        float4 color;
    };

    struct CompositeParams {
        float2 inv_texture_size;
        float blur_sigma;
        float shadow_blur;
        float2 shadow_offset;
        float4 shadow_color;
        uint flags;
    };

    vertex CompositeOut composite_vertex(uint vid [[vertex_id]], const device CompositeVertex* vertices [[buffer(0)]]) {
        CompositeVertex v = vertices[vid];
        CompositeOut out;
        out.position = float4(v.clip_position, 0.0, 1.0);
        out.uv = v.uv;
        out.color = v.color;
        return out;
    }

    constexpr sampler composite_sampler(address::clamp_to_edge, filter::linear);

    float gaussian_weight(float distance, float sigma) {
        if (sigma <= 0.0) {
            return distance == 0.0 ? 1.0 : 0.0;
        }
        return exp(-0.5 * (distance * distance) / (sigma * sigma));
    }

    float4 sample_blur(texture2d<float> content, float2 uv, float sigma, float2 inv_texture_size) {
        if (sigma <= 0.0) {
            return content.sample(composite_sampler, uv);
        }
        constexpr int radius = 4;
        float4 accum = float4(0.0);
        float total = 0.0;
        for (int y = -radius; y <= radius; ++y) {
            for (int x = -radius; x <= radius; ++x) {
                float2 offset = float2(float(x), float(y)) * inv_texture_size * max(sigma, 1.0);
                float weight = gaussian_weight(length(float2(x, y)), sigma * 0.5 + 1.0);
                accum += content.sample(composite_sampler, uv + offset) * weight;
                total += weight;
            }
        }
        return accum / max(total, 0.0001);
    }

    fragment float4 composite_fragment(
        CompositeOut in [[stage_in]],
        texture2d<float> content [[texture(0)]],
        constant CompositeParams& params [[buffer(0)]]
    ) {
        float4 base = (params.flags & 1) != 0
            ? sample_blur(content, in.uv, params.blur_sigma, params.inv_texture_size)
            : content.sample(composite_sampler, in.uv);
        if ((params.flags & 2) != 0) {
            float shadow_sigma = max(params.shadow_blur, params.blur_sigma);
            float2 shadow_uv = in.uv - params.shadow_offset * params.inv_texture_size;
            float shadow_alpha = sample_blur(content, shadow_uv, shadow_sigma, params.inv_texture_size).a;
            float4 shadow = float4(params.shadow_color.rgb, params.shadow_color.a * shadow_alpha);
            base = shadow * (1.0 - base.a) + base;
        }
        return base * in.color;
    }
"#;
