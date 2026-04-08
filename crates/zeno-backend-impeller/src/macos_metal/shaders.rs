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
"#;
