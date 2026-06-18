#version 300 es // 1. 사용할 OpenGL 버전을 선언합니다. 
// "300 es"는 WebGL 2.0 및 모바일(OpenGL ES 3.0)에서 돌아가는 최신 표준을 의미합니다.

// ============================================================================
// [SETTING] 프래그먼트 셰이더의 필수 설정
// ============================================================================
// 소수점 계산의 정밀도를 설정합니다. (highp: 고정밀도, mediump: 중간, lowp: 저정밀도)
// mediump는 모바일 기기나 저사양 GPU에서도 속도가 빠르면서도 눈으로 보기에 충분한 화질을 보장합니다.
precision mediump float;

// ============================================================================
// [INPUT] 스케치 화가(Vertex Shader)와 외부(Rust)에서 받은 데이터들
// ============================================================================
// in: 버텍스 셰이더가 넘겨준 '택배 상자'입니다. 
// 버텍스 셰이더에서 out float vValue; 로 보낸 값을 여기서 똑같은 이름으로 받습니다.
in float vValue;

// uniform: Rust(CPU)에서 일괄적으로 설정해 준 값들입니다.
uniform float uMin;       // 사용자가 설정한 최소값 (예: -2.0m)
uniform float uMax;       // 사용자가 설정한 최대값 (예: 5.0m)
uniform float uAlpha;     // 전체 투명도 (0.0 ~ 1.0)
uniform int uColorMode;   // 현재 선택된 컬러맵 모드 (0:무지개, 1:터보, 2:비리디스 등)

// ============================================================================
// [OUTPUT] 모니터 화면(픽셀)에 최종적으로 칠할 색상
// ============================================================================
// vec4는 (R, G, B, A) 즉, 빨초파 색상과 투명도를 의미합니다.
out vec4 FragColor;

// ============================================================================
// [COLOR PALETTES] 숫자를 색상으로 바꿔주는 공식(함수)들
// ============================================================================

// 0. Rainbow (HSV)
// 조건문(if) 없이 순수 수학 기호(절대값, 소수점 버림 등)만 사용해서 
// 무지개색을 만들어내는 그래픽스 업계의 전설적인 최적화 코드입니다.
vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

// 1. Turbo (Google AI)
// x값(0.0 ~ 1.0)을 받아 6개의 핵심 색상 사이를 부드럽게 섞어줍니다.
vec3 turbo(float x) {
    float r[6] = float[](0.18995, 0.5, 0.8, 1.0, 0.9, 0.5);
    float g[6] = float[](0.07176, 0.5, 0.9, 0.8, 0.3, 0.1);
    float b[6] = float[](0.23217, 0.9, 0.5, 0.1, 0.05, 0.0);
    float pos = clamp(x, 0.0, 1.0) * 5.0;     // x를 0~5 사이의 구간으로 늘림
    int idx = int(min(floor(pos), 4.0));        // 현재 값이 어느 색상 구간(인덱스)에 있는지 찾기
    float t = pos - float(idx);                  // 구간 안에서 얼마나 치우쳐 있는지(비율) 계산

    // mix(A, B, t): A색상과 B색상을 t의 비율만큼 부드럽게 섞어라! (그라데이션 핵심 함수)
    return vec3(mix(r[idx], r[idx + 1], t), mix(g[idx], g[idx + 1], t), mix(b[idx], b[idx + 1], t));
}

// 2. Viridis (Matplotlib Standard)
// 시인성이 가장 좋은 색상 배치 (어두운 남색 -> 노란색)
vec3 viridis(float x) {
    float r[5] = float[](0.267, 0.231, 0.129, 0.369, 0.992);
    float g[5] = float[](0.004, 0.322, 0.569, 0.788, 0.906);
    float b[5] = float[](0.329, 0.545, 0.553, 0.384, 0.145);
    float pos = clamp(x, 0.0, 1.0) * 4.0;
    int idx = int(min(floor(pos), 3.0));
    float t = pos - float(idx);
    return vec3(mix(r[idx], r[idx + 1], t), mix(g[idx], g[idx + 1], t), mix(b[idx], b[idx + 1], t));
}

// 3. Coolwarm (Diverging)
// 양극화 데이터 표현용 (파란색 -> 회색 -> 빨간색)
vec3 coolwarm(float x) {
    float r[5] = float[](0.231, 0.545, 0.867, 0.945, 0.706);
    float g[5] = float[](0.298, 0.631, 0.867, 0.580, 0.016);
    float b[5] = float[](0.753, 0.847, 0.867, 0.514, 0.149);
    float pos = clamp(x, 0.0, 1.0) * 4.0;
    int idx = int(min(floor(pos), 3.0));
    float t = pos - float(idx);
    return vec3(mix(r[idx], r[idx + 1], t), mix(g[idx], g[idx + 1], t), mix(b[idx], b[idx + 1], t));
}

// ============================================================================
// [MAIN] 픽셀 하나를 그릴 때마다 실행되는 메인 함수
// ============================================================================
void main() {
    // 1. 정규화 (Normalization) - "가장 중요한 부분!"
    // 들어온 값(vValue)을 uMin과 uMax 사이의 비율(0.0 ~ 1.0)로 변환합니다.
    // 예: uMin이 -2, uMax가 8일 때 vValue가 3이면, 중간이니까 0.5(50%)가 됩니다.
    // clamp는 혹시라도 값이 범위를 벗어나면 강제로 0.0 이나 1.0으로 잘라냅니다.
    float normalized = clamp((vValue - uMin) / (uMax - uMin), 0.0, 1.0);

    vec3 rgb; // 최종 R, G, B 색상을 담을 빈 통

    // 2. 외부에서 받아온 스위치(uColorMode)에 따라 팔레트 선택
    if(uColorMode == 1) {
        rgb = turbo(normalized);
    } else if(uColorMode == 2) {
        // 값이 낮으면 240도(파랑), 값이 높으면 0도(빨강)에 오도록 각도를 뒤집어 계산합니다.
        float hue = (1.0 - normalized) * 240.0 / 360.0;
        rgb = hsv2rgb(vec3(hue, 1.0, 1.0));
    } else if(uColorMode == 3) {
        rgb = coolwarm(normalized);
    } else if(uColorMode == 4) {
        // 4. Grayscale (흑백)
        // R, G, B에 모두 같은 값을 넣으면 회색이 됩니다! (0이면 검정, 1이면 흰색)
        rgb = vec3(normalized, normalized, normalized);
    } else {
        rgb = viridis(normalized);
    }

    // 3. 최종 색상 출력
    // 계산된 rgb 물감에 외부에서 받아온 투명도(uAlpha)를 합쳐서 픽셀에 칠하고 끝냅니다!
    FragColor = vec4(rgb, uAlpha);
}