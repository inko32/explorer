// minimal uinput virtual mouse: inject <cmd> [x y]
// cmds: move x y | down | up | click x y
#include <fcntl.h>
#include <linux/uinput.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/ioctl.h>

static int fd;
static void emit(int type, int code, int val) {
    struct input_event e = {0};
    e.type = type; e.code = code; e.value = val;
    write(fd, &e, sizeof(e));
}
static void sync_(void) { emit(EV_SYN, SYN_REPORT, 0); }
static void move(int x, int y) {
    emit(EV_ABS, ABS_X, x); emit(EV_ABS, ABS_Y, y); sync_();
}
static void btn(int v) { emit(EV_KEY, BTN_LEFT, v); sync_(); }
static void msleep(int ms) { usleep(ms * 1000); }

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    fd = open("/dev/uinput", O_WRONLY | O_NONBLOCK);
    if (fd < 0) { perror("uinput"); return 1; }
    ioctl(fd, UI_SET_EVBIT, EV_ABS);
    ioctl(fd, UI_SET_ABSBIT, ABS_X);
    ioctl(fd, UI_SET_ABSBIT, ABS_Y);
    ioctl(fd, UI_SET_EVBIT, EV_KEY);
    ioctl(fd, UI_SET_KEYBIT, BTN_LEFT);
    struct uinput_user_dev d = {0};
    strcpy(d.name, "spike-injector");
    d.id.bustype = BUS_USB;
    d.id.vendor = 0x1234; d.id.product = 0x5678;
    d.absmin[ABS_X] = 0; d.absmax[ABS_X] = 3839;
    d.absmin[ABS_Y] = 0; d.absmax[ABS_Y] = 2159;
    write(fd, &d, sizeof(d));
    ioctl(fd, UI_DEV_CREATE);
    msleep(400); // let the compositor pick it up

    if (!strcmp(argv[1], "move")) {
        move(atoi(argv[2]), atoi(argv[3]));
    } else if (!strcmp(argv[1], "down")) {
        btn(1);
    } else if (!strcmp(argv[1], "up")) {
        btn(0);
    } else if (!strcmp(argv[1], "click")) {
        move(atoi(argv[2]), atoi(argv[3])); msleep(80);
        btn(1); msleep(80); btn(0);
    } else if (!strcmp(argv[1], "drag")) {
        // drag x1 y1 x2 y2 [steps]
        int x1 = atoi(argv[2]), y1 = atoi(argv[3]), x2 = atoi(argv[4]), y2 = atoi(argv[5]);
        int steps = argc > 6 ? atoi(argv[6]) : 20;
        move(x1, y1); msleep(120);
        btn(1); msleep(120);
        for (int i = 1; i <= steps; i++) {
            move(x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps);
            msleep(16);
        }
        msleep(120);
        btn(0);
    }
    msleep(200);
    ioctl(fd, UI_DEV_DESTROY);
    close(fd);
    return 0;
}
