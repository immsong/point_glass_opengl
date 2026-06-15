import 'package:flutter/material.dart';

/// 3D 공간의 원점(0,0,0)에 X, Y, Z 축을 표시하기 위한 설정 모델
class PointGlassOpenGLAxis {
  final bool enable;

  // 축의 시각적 크기
  final double length; // 축 선의 길이 (예: 기본값 1.0m)
  final double lineWidth; // 축 선의 두께

  // X, Y, Z 축의 색상 (3D 그래픽스 표준 관례: RGB)
  final Color colorX; // Red
  final Color colorY; // Green
  final Color colorZ; // Blue

  // 💡 방금 만든 강력한 라벨 시스템 활용!
  final bool enableLabel; // 축 끝에 "X", "Y", "Z" 글씨 표시 여부
  final double labelFontSize; // 라벨 글씨 크기

  PointGlassOpenGLAxis({
    this.enable = true,
    this.length = 1.0,
    this.lineWidth = 1.0, // Grid(1.0)보다 살짝 두껍게 해서 눈에 띄게 함
    // 기본값은 3D 업계 표준 색상 (X:빨강, Y:초록, Z:파랑)
    this.colorX = const Color.fromARGB(255, 255, 50, 50),
    this.colorY = const Color.fromARGB(255, 50, 255, 50),
    this.colorZ = const Color.fromARGB(255, 50, 100, 255), // Z축은 파랑

    this.enableLabel = true,
    this.labelFontSize = 14.0,
  });
}
