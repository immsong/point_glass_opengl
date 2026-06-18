import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'package:vector_math/vector_math_64.dart' as vm;

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

    if (result.isEmpty) {
      return Float32List.fromList([
        // dummy point
        0.0, 0.0, 0.0,
      ]);
    }

    return Float32List.fromList(result);
  }

  /// Grid 설정값을 OpenGL Lines용 데이터로 변환
  /// 포맷: [X, Y, Z, R, G, B, A, Thickness]
  static Float32List convertGrid(PointGlassOpenGLGrid grid) {
    if (!grid.enable) {
      return Float32List.fromList([
        // dummy line
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // 시작점
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // 끝점
      ]);
    }

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

  /// 축(Axis) 설정값을 면(Polygons/Triangles) 데이터로 변환하여 두께를 구현
  /// 포맷: [X, Y, Z, R, G, B, A, 1.0] (8 floats per vertex)
  static Float32List convertAxisToPolygons(PointGlassOpenGLAxis axis) {
    if (!axis.enable) {
      return Float32List.fromList([
        // dummy polygon
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // 꼭짓점 1
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // 꼭짓점 2
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // 꼭짓점 3
      ]);
    }
    ;

    final List<double> result = [];

    // 두께 설정 (예: lineWidth가 2.0이면 0.02m 두께의 상자로 만듦)
    // 3D 공간의 스케일에 맞게 적절히 조절 (필요시 곱하는 비율 조정 가능)
    final double w = axis.lineWidth * 0.01;

    // 사각형(Quad) 면 하나를 2개의 삼각형(Triangle)으로 쪼개서 넣는 헬퍼 함수
    void addQuad(
      vm.Vector3 p1,
      vm.Vector3 p2,
      vm.Vector3 p3,
      vm.Vector3 p4,
      Color c,
    ) {
      void addVertex(vm.Vector3 p) {
        result.addAll([
          p.x,
          p.y,
          p.z,
          c.r,
          c.g,
          c.b,
          c.a,
          1.0,
        ]); // 8자리 포맷 (마지막은 padding/w)
      }

      // 첫 번째 삼각형 (p1, p2, p3)
      addVertex(p1);
      addVertex(p2);
      addVertex(p3);
      // 두 번째 삼각형 (p1, p3, p4)
      addVertex(p1);
      addVertex(p3);
      addVertex(p4);
    }

    // 직육면체(Cuboid)를 생성하는 헬퍼 함수 (6개의 면 = 12개의 삼각형)
    void addCuboid(
      double xMin,
      double xMax,
      double yMin,
      double yMax,
      double zMin,
      double zMax,
      Color c,
    ) {
      final v000 = vm.Vector3(xMin, yMin, zMin);
      final v100 = vm.Vector3(xMax, yMin, zMin);
      final v110 = vm.Vector3(xMax, yMax, zMin);
      final v010 = vm.Vector3(xMin, yMax, zMin);
      final v001 = vm.Vector3(xMin, yMin, zMax);
      final v101 = vm.Vector3(xMax, yMin, zMax);
      final v111 = vm.Vector3(xMax, yMax, zMax);
      final v011 = vm.Vector3(xMin, yMax, zMax);

      // 앞, 뒤, 좌, 우, 위, 아래 6개의 면(Quad) 생성
      addQuad(v001, v101, v111, v011, c); // Front
      addQuad(v100, v000, v010, v110, c); // Back
      addQuad(v000, v001, v011, v010, c); // Left
      addQuad(v101, v100, v110, v111, c); // Right
      addQuad(v011, v111, v110, v010, c); // Top
      addQuad(v000, v100, v101, v001, c); // Bottom
    }

    // 1. X축 (Red): 길이는 X로 길게, Y와 Z는 두께(w)만큼
    addCuboid(0.0, axis.length, -w, w, -w, w, axis.colorX);

    // 2. Y축 (Green): 길이는 Y로 길게, X와 Z는 두께(w)만큼
    addCuboid(-w, w, 0.0, axis.length, -w, w, axis.colorY);

    // 3. Z축 (Blue): 길이는 Z로 길게, X와 Y는 두께(w)만큼
    addCuboid(-w, w, -w, w, 0.0, axis.length, axis.colorZ);

    return Float32List.fromList(result);
  }
}
