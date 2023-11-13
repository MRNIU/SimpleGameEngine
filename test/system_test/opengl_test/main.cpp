
/**
 * @file main.cpp
 * @brief main 实现
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-10-31
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimplePhysicsEngine
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-10-31<td>Zone.N<td>迁移到 doxygen
 * </table>
 */

#include <cmath>

#include <GL/freeglut.h>
#include <GL/gl.h>

int n = 3;

static void DisplayShape(void) {
  glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
  glColor3d(1, 0.1, 0.6);

  const float x0 = 0.0f;
  const float y0 = 0.0f;
  const float sideLen = 0.5;
  float dist = sideLen / 2.0f / sin(M_PI * 2.0f / n / 2.0f);
  float startAngle = -M_PI * (n - 2) / 2 / n;

  glBegin(GL_LINE_LOOP);

  for (int i = 0; i < n; ++i) // n - sides count
  {
    float sideAngle = M_PI * 2.0 * i / n + startAngle;
    float x = x0 + dist * cos(sideAngle);
    float y = y0 + dist * sin(sideAngle);
    glVertex2f(x, y);
  }
  glEnd();

  glutSwapBuffers();
  glutPostRedisplay();
}

static void key(unsigned char key, int x, int y) {
  switch (key) {
  case 27:
  case 'q': // quit
    exit(0);
    break;

  case '1': // increase sides count
    n++;
    break;

  case '2':    // decrease sides count
    if (n > 3) // cannot be less than 3
      n--;
    break;
  }
  glutPostRedisplay();
}

static void reshape(int width, int height) {
  glViewport(0, 0, width, height);
  glMatrixMode(GL_PROJECTION);
  glLoadIdentity();
  double aspect = (double)width / height;
  glOrtho(-aspect, aspect, -1.0, 1.0, -1.0, 1.0);
  glMatrixMode(GL_MODELVIEW);
}

int main(int argc, char *argv[]) {
  glutInit(&argc, argv);
  glutInitWindowSize(640, 480);
  glutInitWindowPosition(10, 10);
  glutInitDisplayMode(GLUT_RGB | GLUT_DOUBLE | GLUT_DEPTH);

  glutCreateWindow("GLUT Shapes");

  glutReshapeFunc(reshape);
  glutDisplayFunc(DisplayShape);
  glutKeyboardFunc(key);

  glutMainLoop();

  return EXIT_SUCCESS;
}
