enum PointGlassOpenGLPointsColorMode {
  viridis(0, 'viridis'),
  viridis2(1, 'viridis2'),
  turbo(2, 'turbo'),
  turbo2(3, 'turbo2'),
  rainbow(4, 'rainbow'),
  rainbow2(5, 'rainbow2'),
  coolwarm(6, 'coolwarm'),
  coolwarm2(7, 'coolwarm2'),
  grayscale(8, 'grayscale');

  final int value;
  final String label;

  const PointGlassOpenGLPointsColorMode(this.value, this.label);
}
