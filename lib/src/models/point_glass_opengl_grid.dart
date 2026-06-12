import 'package:flutter/material.dart';

/// 바닥 평면 격자(Grid)를 정의하는 데이터 모델
class PointGlassOpenGLGrid {
  final bool enable;
  final double gridSize; // 격자 전체 크기 (예: 20 -> -10 ~ 10 범위)
  final double gridStep; // 격자 한 칸의 간격 (예: 1)
  final Color color;
  final double lineWidth;

  final bool enableLabel;
  final Color labelColor;
  final double labelFontSize;

  PointGlassOpenGLGrid({
    this.enable = true,
    this.gridSize = 20.0,
    this.gridStep = 1.0,
    this.color = const Color(0xC8248EFF), // 기본값: 반투명한 파란색
    this.lineWidth = 1.0,
    this.enableLabel = true,
    this.labelColor = Colors.white70,
    this.labelFontSize = 12,
  });
}
