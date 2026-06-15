import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_points.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_grid.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_axis.dart';

class DataConverter {
  /// PointGlassOpenGLPoints 그룹 배열을 OpenGL용 Float32List로 변환
  /// 포맷: [X, Y, Z, R, G, B, A, Size]
  static Float32List convertPointsGroup(List<PointGlassOpenGLPoints> groups) {
    final List<double> result = [];

    for (final group in groups) {
      if (!group.enable) {
        continue;
      }

      final size = group.pointSize;
      for (final pt in group.points) {
        result.addAll([
          pt.x,
          pt.y,
          pt.z,
          group.color.r,
          group.color.g,
          group.color.b,
          group.color.a,
          size,
        ]);
      }
    }

    return Float32List.fromList(result);
  }

  /// Grid 설정값을 OpenGL Lines용 데이터로 변환
  /// 포맷: [X, Y, Z, R, G, B, A, Thickness]
  static Float32List convertGrid(PointGlassOpenGLGrid grid) {
    if (!grid.enable) return Float32List(0);

    final List<double> result = [];

    // gridSize가 20, gridStep이 1이면 count는 20칸.
    // 절반(half)은 10이 되어, -10부터 +10까지 선을 긋습니다.
    final int count = (grid.gridSize / grid.gridStep).round();
    final double half = grid.gridSize / 2.0;

    final r = grid.color.r;
    final g = grid.color.g;
    final b = grid.color.b;
    final a = grid.color.a;
    final w = grid.lineWidth;

    for (int i = 0; i <= count; i++) {
      double t = -half + (i * grid.gridStep);

      // 가로선 (X축에 평행하게 쭉 긋고, Y축(t) 위치를 이동)
      result.addAll([-half, t, 0.0, r, g, b, a, w]); // 시작점
      result.addAll([half, t, 0.0, r, g, b, a, w]); // 끝점

      // 세로선 (Y축에 평행하게 쭉 긋고, X축(t) 위치를 이동)
      result.addAll([t, -half, 0.0, r, g, b, a, w]); // 시작점
      result.addAll([t, half, 0.0, r, g, b, a, w]); // 끝점
    }

    return Float32List.fromList(result);
  }

  /// Axis 설정값을 OpenGL Lines용 데이터로 변환
  /// 포맷: [X, Y, Z, R, G, B, A, Thickness]
  static Float32List convertAxis(PointGlassOpenGLAxis axis) {
    if (!axis.enable) return Float32List(0);
    final List<double> result = [];
    final double w = axis.lineWidth;

    // 색상을 0.0 ~ 1.0 사이의 Float으로 변환하는 헬퍼 함수
    void addLine(
      double x1,
      double y1,
      double z1,
      double x2,
      double y2,
      double z2,
      Color c,
    ) {
      result.addAll([x1, y1, z1, c.r, c.g, c.b, c.a, w]);
      result.addAll([x2, y2, z2, c.r, c.g, c.b, c.a, w]);
    }

    // X축 (Red)
    addLine(0, 0, 0, axis.length, 0, 0, axis.colorX);
    // Y축 (Green)
    addLine(0, 0, 0, 0, axis.length, 0, axis.colorY);
    // Z축 (Blue)
    addLine(0, 0, 0, 0, 0, axis.length, axis.colorZ);

    return Float32List.fromList(result);
  }
}
