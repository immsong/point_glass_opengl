#version 330 core

// ============================================================================
// points.frag
// ============================================================================
//
// 이 파일은 Point Cloud 점의 "색상"을 계산하는 shader.
//
// OpenGL 렌더링 파이프라인에서 Fragment Shader 단계에 해당.
// 쉽게 말하면 화면에 찍힐 pixel/fragment의 최종 색을 결정함.
//
// points.vert에서 넘겨받은 vValue를 기준으로,
// viridis / turbo / rainbow / coolwarm / grayscale 같은 컬러맵을 적용함.

// ============================================================================
// [INPUT] Vertex Shader에서 넘어온 값
// ============================================================================
//
// points.vert에서:
//
// out float vValue;
//
// 로 내보낸 값을 여기서 받음.
//
// OpenGL이 vertex 사이의 값은 자동으로 보간(interpolation)하지만,
// GL_POINTS에서는 점 단위로 거의 그대로 들어온다고 보면 됨.
in float vValue;

// ============================================================================
// [UNIFORM] Rust/Dart에서 설정하는 공통 값
// ============================================================================
//
// value를 색상으로 바꾸기 위한 범위.
//
// 예:
// uMin = -2.0
// uMax =  5.0
//
// vValue가 uMin에 가까우면 컬러맵의 시작 색,
// uMax에 가까우면 컬러맵의 끝 색이 됨.
uniform float uMin;
uniform float uMax;

// 전체 point cloud 투명도.
//
// 0.0 = 완전 투명
// 1.0 = 완전 불투명
uniform float uAlpha;

// 컬러맵 선택 값.
//
// 0: Viridis 1
// 1: Viridis 2
// 2: Turbo 1
// 3: Turbo 2
// 4: Rainbow 1
// 5: Rainbow 2
// 6: Coolwarm 1
// 7: Coolwarm 2
// 8: Grayscale
//
// 내부적으로 각 컬러맵의 1/2는 색상 방향이 반대.
uniform int uColorMode;

// ============================================================================
// [OUTPUT] 최종 fragment 색상
// ============================================================================
//
// vec4 = RGBA
//
// R: red
// G: green
// B: blue
// A: alpha
//
// 각 값은 보통 0.0 ~ 1.0 범위.
out vec4 FragColor;

// ============================================================================
// [COLOR MAP 0] HSV → RGB
// ============================================================================
//
// HSV 색상 모델:
//
// H: hue, 색상 각도
// S: saturation, 채도
// V: value, 밝기
//
// 이 함수는 HSV 값을 RGB로 바꿔줌.
//
// 여기서는 rainbow color map을 만들 때 사용.
// normalized 값이 낮으면 파란색 계열,
// 높으면 빨간색 계열로 가도록 hue를 계산함.
vec3 hsv2rgb(vec3 c) {
    // K는 HSV → RGB 변환에서 사용하는 상수 묶음.
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);

    // fract:
    // 소수 부분만 가져옴.
    //
    // abs / clamp / mix 조합으로 조건문 없이 RGB 색상을 계산.
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);

    // c.z = value, 밝기
    // c.y = saturation, 채도
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

// ============================================================================
// [COLOR MAP 1] Turbo
// ============================================================================
//
// Google Turbo 컬러맵을 단순화한 버전.
//
// x는 0.0 ~ 1.0 사이의 정규화된 값.
// 내부적으로 6개의 anchor color 사이를 mix로 보간함.
vec3 turbo(float x) {
    // 색상 anchor point.
    // r/g/b 각각 6개 지점을 가지고 있음.
    float r[6] = float[](0.18995, 0.5, 0.8, 1.0, 0.9, 0.5);
    float g[6] = float[](0.07176, 0.5, 0.9, 0.8, 0.3, 0.1);
    float b[6] = float[](0.23217, 0.9, 0.5, 0.1, 0.05, 0.0);

    // x를 0~5 구간으로 확장.
    //
    // 예:
    // x = 0.0 → pos = 0.0
    // x = 0.5 → pos = 2.5
    // x = 1.0 → pos = 5.0
    float pos = clamp(x, 0.0, 1.0) * 5.0;

    // 현재 pos가 어느 구간에 있는지 계산.
    //
    // idx는 최대 4까지만 허용.
    // 이유:
    // 아래에서 idx + 1을 접근하므로 idx가 5가 되면 배열 범위를 넘음.
    int idx = int(min(floor(pos), 4.0));

    // 현재 구간 내부에서의 보간 비율.
    //
    // 예:
    // pos = 2.25
    // idx = 2
    // t = 0.25
    float t = pos - float(idx);

    // idx 색상과 idx+1 색상을 t 비율로 섞음.
    return vec3(mix(r[idx], r[idx + 1], t), mix(g[idx], g[idx + 1], t), mix(b[idx], b[idx + 1], t));
}

// ============================================================================
// [COLOR MAP 2] Viridis
// ============================================================================
//
// Matplotlib에서 많이 쓰는 perceptually uniform 계열 컬러맵.
//
// 장점:
// - 값 차이가 비교적 균일하게 느껴짐
// - 색약 친화적이라고 알려진 편
// - 어두운 보라/남색 → 초록 → 노랑 흐름
//
// 여기서는 간단한 5-point 보간 버전.
vec3 viridis(float x) {
    float r[5] = float[](0.267, 0.231, 0.129, 0.369, 0.992);
    float g[5] = float[](0.004, 0.322, 0.569, 0.788, 0.906);
    float b[5] = float[](0.329, 0.545, 0.553, 0.384, 0.145);

    // 5개 anchor color → 4개 구간
    float pos = clamp(x, 0.0, 1.0) * 4.0;

    // idx는 0~3만 사용.
    // idx + 1 접근 때문에 최대 3.
    int idx = int(min(floor(pos), 3.0));

    float t = pos - float(idx);

    return vec3(mix(r[idx], r[idx + 1], t), mix(g[idx], g[idx + 1], t), mix(b[idx], b[idx + 1], t));
}

// ============================================================================
// [COLOR MAP 3] Coolwarm
// ============================================================================
//
// Diverging color map.
// 낮은 값은 파란색, 중간은 밝은색/회색, 높은 값은 빨간색 계열.
//
// 기준값보다 낮음/높음이 중요한 데이터 표현에 유용.
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
// [MAIN]
// ============================================================================
//
// Point Cloud의 각 점에 적용할 최종 색상을 계산.
//
// Color Mode:
// 0, 1 : Viridis 1/2
// 2, 3 : Turbo 1/2
// 4, 5 : Rainbow 1/2
// 6, 7 : Coolwarm 1/2
// 8    : Grayscale

void main() {
    // ------------------------------------------------------------------------
    // 1. Value 정규화
    // ------------------------------------------------------------------------

    float range = max(abs(uMax - uMin), 0.000001);
    float normalized = clamp((vValue - uMin) / range, 0.0, 1.0);

    // ------------------------------------------------------------------------
    // 2. Color Mode 해석
    // ------------------------------------------------------------------------
    //
    // 짝수: Color Map 1
    // 홀수: Color Map 2 (색상 방향 반전)
    //
    // Grayscale(8)은 반전하지 않음.

    if(uColorMode < 8 && (uColorMode % 2) == 1) {
        normalized = 1.0 - normalized;
    }

    // 기존 컬러맵 표시 범위 유지 (20% ~ 100%)
    float displayValue = mix(0.20, 1.0, normalized);

    // 컬러맵 그룹 선택
    int colorMap = uColorMode / 2;

    // ------------------------------------------------------------------------
    // 3. Color Map 적용
    // ------------------------------------------------------------------------

    vec3 rgb;

    if(colorMap == 1) {
        // Turbo 1 / Turbo 2
        rgb = turbo(displayValue);

    } else if(colorMap == 2) {
        // Rainbow 1 / Rainbow 2
        float hue = (1.0 - displayValue) * 240.0 / 360.0;
        rgb = hsv2rgb(vec3(hue, 1.0, 1.0));

    } else if(colorMap == 3) {
        // Coolwarm 1 / Coolwarm 2
        rgb = coolwarm(displayValue);

    } else if(colorMap == 4) {
        // Grayscale
        rgb = vec3(displayValue);

    } else {
        // Viridis 1 / Viridis 2
        rgb = viridis(displayValue);
    }

    // ------------------------------------------------------------------------
    // 4. 최종 색상 출력
    // ------------------------------------------------------------------------

    FragColor = vec4(rgb, uAlpha);
}