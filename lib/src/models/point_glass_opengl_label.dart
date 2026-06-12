import 'package:flutter/material.dart';
import 'package:vector_math/vector_math.dart' as vm;

/// 3D 공간 상의 특정 위치에 띄울 텍스트 라벨 파라미터
class PointGlassOpenGLLabel {
  final vm.Vector3 position; // 3D 좌표
  final String text; // 화면에 표시할 글씨
  final TextStyle? style; // 폰트 크기, 색상 등

  PointGlassOpenGLLabel({
    required this.position,
    required this.text,
    this.style,
  });
}
