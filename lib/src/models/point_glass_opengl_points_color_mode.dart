enum PointGlassOpenGLPointsColorMode {
  viridis(0, 'viridis'),
  turbo(1, 'turbo'),
  rainbow(2, 'rainbow'),
  coolwarm(3, 'coolwarm'),
  grayscale(4, 'grayscale');

  final int value;
  final String label;

  const PointGlassOpenGLPointsColorMode(this.value, this.label);
}
