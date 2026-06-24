#version 300 es

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
// [PRECISION]
// ============================================================================
//
// OpenGL ES shader에서는 float 정밀도를 명시해야 함.
//
// mediump:
// - 모바일/임베디드 GPU에서 성능과 품질의 균형이 좋음
// - 색상 계산 정도에는 대부분 충분
//
// desktop OpenGL용 #version 330 core shader로 분리할 경우
// precision mediump float; 는 제거하는 쪽이 안전함.
precision mediump float;

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
// 현재 코드 기준:
//
// 1: turbo
// 2: rainbow HSV
// 3: coolwarm
// 4: grayscale
// 그 외: viridis
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
    vec4 K = vec4(1.0f, 2.0f / 3.0f, 1.0f / 3.0f, 3.0f);

    // fract:
    // 소수 부분만 가져옴.
    //
    // abs / clamp / mix 조합으로 조건문 없이 RGB 색상을 계산.
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0f - K.www);

    // c.z = value, 밝기
    // c.y = saturation, 채도
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0f, 1.0f), c.y);
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
    float r[6] = float[](0.18995f, 0.5f, 0.8f, 1.0f, 0.9f, 0.5f);
    float g[6] = float[](0.07176f, 0.5f, 0.9f, 0.8f, 0.3f, 0.1f);
    float b[6] = float[](0.23217f, 0.9f, 0.5f, 0.1f, 0.05f, 0.0f);

    // x를 0~5 구간으로 확장.
    //
    // 예:
    // x = 0.0 → pos = 0.0
    // x = 0.5 → pos = 2.5
    // x = 1.0 → pos = 5.0
    float pos = clamp(x, 0.0f, 1.0f) * 5.0f;

    // 현재 pos가 어느 구간에 있는지 계산.
    //
    // idx는 최대 4까지만 허용.
    // 이유:
    // 아래에서 idx + 1을 접근하므로 idx가 5가 되면 배열 범위를 넘음.
    int idx = int(min(floor(pos), 4.0f));

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
    float r[5] = float[](0.267f, 0.231f, 0.129f, 0.369f, 0.992f);
    float g[5] = float[](0.004f, 0.322f, 0.569f, 0.788f, 0.906f);
    float b[5] = float[](0.329f, 0.545f, 0.553f, 0.384f, 0.145f);

    // 5개 anchor color → 4개 구간
    float pos = clamp(x, 0.0f, 1.0f) * 4.0f;

    // idx는 0~3만 사용.
    // idx + 1 접근 때문에 최대 3.
    int idx = int(min(floor(pos), 3.0f));

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
    float r[5] = float[](0.231f, 0.545f, 0.867f, 0.945f, 0.706f);
    float g[5] = float[](0.298f, 0.631f, 0.867f, 0.580f, 0.016f);
    float b[5] = float[](0.753f, 0.847f, 0.867f, 0.514f, 0.149f);

    float pos = clamp(x, 0.0f, 1.0f) * 4.0f;
    int idx = int(min(floor(pos), 3.0f));
    float t = pos - float(idx);

    return vec3(mix(r[idx], r[idx + 1], t), mix(g[idx], g[idx + 1], t), mix(b[idx], b[idx + 1], t));
}

// ============================================================================
// [MAIN]
// ============================================================================
//
// 점 하나의 최종 색상을 결정.
void main() {
    // ------------------------------------------------------------------------
    // 1. vValue 정규화
    // ------------------------------------------------------------------------
    //
    // raw value를 0.0 ~ 1.0 범위로 변환.
    //
    // normalized = 0.0 → 컬러맵 시작 색
    // normalized = 1.0 → 컬러맵 끝 색
    //
    // 기존 코드:
    //
    // (vValue - uMin) / (uMax - uMin)
    //
    // 문제:
    // uMax == uMin이면 0으로 나누게 됨.
    //
    // 그래서 range에 최소값을 둬서 방어.
    float range = max(abs(uMax - uMin), 0.000001f);
    float normalized = clamp((vValue - uMin) / range, 0.0f, 1.0f);

    // 최종 RGB 값
    vec3 rgb;

    // ------------------------------------------------------------------------
    // 2. 컬러맵 선택
    // ------------------------------------------------------------------------
    //
    // uColorMode 값에 따라 다른 색상 함수 사용.
    if(uColorMode == 1) {
        // Turbo
        rgb = turbo(normalized);
    } else if(uColorMode == 2) {
        // Rainbow HSV
        //
        // hue 240도 = 파란색
        // hue   0도 = 빨간색
        //
        // normalized가 낮으면 파랑,
        // 높으면 빨강이 되도록 1.0 - normalized 사용.
        float hue = (1.0f - normalized) * 240.0f / 360.0f;
        rgb = hsv2rgb(vec3(hue, 1.0f, 1.0f));
    } else if(uColorMode == 3) {
        // Coolwarm
        rgb = coolwarm(normalized);
    } else if(uColorMode == 4) {
        // Grayscale
        //
        // R, G, B가 같으면 회색.
        rgb = vec3(normalized, normalized, normalized);
    } else {
        // 기본 컬러맵: Viridis
        rgb = viridis(normalized);
    }

    // ------------------------------------------------------------------------
    // 3. 최종 색상 출력
    // ------------------------------------------------------------------------
    //
    // RGB는 위에서 계산한 값.
    // Alpha는 uniform으로 받은 전체 투명도.
    FragColor = vec4(rgb, uAlpha);
}