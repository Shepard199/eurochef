precision mediump float;

in vec2 f_uv;
in vec2 f_normalUv;
in vec4 f_color;
in vec3 f_eye;
in vec3 f_worldPos;
in vec3 f_worldNormal;

uniform sampler2D u_texture;
uniform float u_cutoutThreshold;
uniform vec4 u_tint;
uniform int u_globalLightingEnabled;
uniform vec3 u_globalLightDirection[3];
uniform vec3 u_globalLightColour[3];
uniform vec3 u_globalAmbient;
uniform int u_globalLightmapEnabled;
uniform sampler2D u_globalLightmap;
uniform vec2 u_globalLightmapMin;
uniform vec2 u_globalLightmapSpan;
uniform vec4 u_globalLightmapCoefficients;

const int EC_MAX_NATIVE_LIGHTS = 16;
uniform int u_nativeLightCount;
uniform float u_nativeLightStrength;
uniform vec4 u_nativeLightPositionRadius[EC_MAX_NATIVE_LIGHTS];
uniform vec4 u_nativeLightDirectionType[EC_MAX_NATIVE_LIGHTS];
uniform vec4 u_nativeLightColorEffect[EC_MAX_NATIVE_LIGHTS];
uniform vec2 u_nativeLightParameters[EC_MAX_NATIVE_LIGHTS];

vec2 matcap(vec3 eye, vec3 normal) {
  vec3 reflected = reflect(eye, normal);
  float m = 2.8284271247461903 * sqrt( reflected.z+1.0 );
  return reflected.xy / m + 0.5;
}

bool nativeLightHasFeature(float lightType, float featureBit) {
  return mod(floor(lightType / featureBit), 2.0) > 0.5;
}

float nativeLightRangeFactor(float distanceToLight, float radius, float fullEffectFraction) {
  if (radius <= 0.000001) return 0.0;
  float normalizedDistance = distanceToLight / radius;
  if (normalizedDistance >= 1.0) return 0.0;
  if (normalizedDistance <= fullEffectFraction) return 1.0;
  float fadeRange = 1.0 - fullEffectFraction;
  if (fadeRange <= 0.000001) return 1.0;
  return max((1.0 - normalizedDistance) / fadeRange, 0.0);
}

float nativeLightConeFactor(vec3 direction, vec3 pointFromLight, float beamAngleDegrees) {
  float distanceFromLight = length(pointFromLight);
  float angleFraction = beamAngleDegrees / 180.0;
  if (distanceFromLight <= 0.000001 || angleFraction <= 0.000001) return 0.0;
  float alignment = dot(direction, pointFromLight / distanceFromLight);
  float factor = 1.0 + (alignment - 1.0) / angleFraction;
  return factor >= 0.003882353 ? min(factor, 1.0) : 0.0;
}

float nativeLightPositiveDot(float value) {
  return value >= 0.003882353 ? value : 0.0;
}

vec3 nativeLighting(vec3 normal) {
  vec3 accumulated = vec3(0.0);
  for (int i = 0; i < EC_MAX_NATIVE_LIGHTS; ++i) {
    if (i >= u_nativeLightCount) break;

    vec4 positionRadius = u_nativeLightPositionRadius[i];
    vec4 directionType = u_nativeLightDirectionType[i];
    vec4 colorEffect = u_nativeLightColorEffect[i];
    float lightType = directionType.w;
    vec3 direction = directionType.xyz;
    vec3 pointFromLight = f_worldPos - positionRadius.xyz;
    vec3 toLight = -pointFromLight;
    float factor = 1.0;

    // EXGeoLight::ltype is a feature mask consumed by Robots.exe 0x005551A0:
    // 0x1 range, 0x2 position/normal, 0x4 beam cone, 0x8 beam/normal.
    if (nativeLightHasFeature(lightType, 1.0)) {
      factor *= nativeLightRangeFactor(length(pointFromLight), positionRadius.w, colorEffect.a);
    }
    if (nativeLightHasFeature(lightType, 4.0)) {
      factor *= nativeLightConeFactor(direction, pointFromLight, u_nativeLightParameters[i].x);
    }
    if (nativeLightHasFeature(lightType, 8.0)) {
      factor *= nativeLightPositiveDot(dot(normal, -direction));
    }
    if (nativeLightHasFeature(lightType, 2.0)) {
      float toLightLength = length(toLight);
      factor *= toLightLength > 0.000001
        ? nativeLightPositiveDot(dot(normal, toLight / toLightLength))
        : 0.0;
    }
    accumulated += colorEffect.rgb * factor;
  }
  return accumulated * u_nativeLightStrength;
}

out vec4 o_color;
void main() {
#ifdef EC_MATCAP
    o_color = texture2D(u_texture, f_normalUv) * f_color;
    return;
#endif

    vec4 texel = texture(u_texture, f_uv);
    if(texel.a <= u_cutoutThreshold) discard;

    vec3 worldNormal = normalize(f_worldNormal);
#ifdef EC_NO_VERTEX_LIGHTING
    o_color = vec4(texel.rgb * u_tint.rgb, texel.a * f_color.a * u_tint.a);
#else
    vec4 vertexBase = texel * f_color;
    if (u_globalLightingEnabled != 0) {
        vec3 globalLight = u_globalAmbient;
        if (u_globalLightmapEnabled != 0) {
            vec2 uv = (f_worldPos.xz - u_globalLightmapMin) / u_globalLightmapSpan;
            vec3 sampled = texture(u_globalLightmap, clamp(uv, 0.0, 1.0)).rgb;
            vec3 transformed = sampled * u_globalLightmapCoefficients.z;
            float energy = length(transformed);
            if (energy > u_globalLightmapCoefficients.y && energy > 0.0) {
                transformed *= u_globalLightmapCoefficients.y / energy;
            }
            energy = length(transformed);
            transformed = transformed * u_globalLightmapCoefficients.w
                + vec3(energy * 0.57735026) * (1.0 - u_globalLightmapCoefficients.w);
            globalLight = max(transformed, vec3(u_globalLightmapCoefficients.x));
        }
        for (int i = 0; i < 3; ++i) {
            globalLight += u_globalLightColour[i]
                * max(dot(worldNormal, u_globalLightDirection[i]), 0.0);
        }
        // u_tint is the legacy renderer's material/pass globalDiffuse multiplier.
        o_color = vec4(vertexBase.rgb * globalLight * u_tint.rgb, vertexBase.a * u_tint.a);
    } else {
        o_color = vertexBase * u_tint;
        // The direct local-light diagnostic is intentionally after globalDiffuse,
        // matching the original positional-light composition order.
        o_color.rgb += texel.rgb * nativeLighting(worldNormal);
    }
#endif

#ifdef EC_NO_TRANSPARENCY
    o_color.a = 1.0;
#endif
}
