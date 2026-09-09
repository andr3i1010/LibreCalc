// SPDX-License-Identifier: GPL-3.0-only

#include <QApplication>
#include <QDir>
#include <QEventPoint>
#include <QFileDialog>
#include <QFileInfo>
#include <QHash>
#include <QImage>
#include <QKeyEvent>
#include <QLinearGradient>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QProcess>
#include <QRadialGradient>
#include <QRegularExpression>
#include <QSet>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTimer>
#include <QTouchEvent>
#include <QWidget>

#include <array>
#include <iostream>

namespace {

constexpr qreal CanvasWidth = 1130.0;
constexpr qreal CanvasHeight = 2340.0;
const QRectF ScreenRect(167.0, 173.0, 796.0, 597.0);
const QRectF ScreenshotRect(451.0, 2228.0, 82.0, 82.0);
const QRectF FullscreenRect(597.0, 2228.0, 82.0, 82.0);

enum class KeyStyle { Dpad, Round, Home, Power, Small, Large };

struct KeyDefinition {
    const char *name;
    const char *main;
    const char *orange;
    const char *alpha;
    QRectF rect;
    KeyStyle style;
};

// Physical geometry and electrical names are shared by painting and input.
const std::array<KeyDefinition, 46> Keys = {{
    {"Left", "", "", "", {116, 982, 126, 85}, KeyStyle::Dpad},
    {"Up", "", "", "", {212, 884, 85, 126}, KeyStyle::Dpad},
    {"Down", "", "", "", {212, 1038, 85, 126}, KeyStyle::Dpad},
    {"Right", "", "", "", {271, 982, 126, 85}, KeyStyle::Dpad},
    {"OK", "OK", "", "", {735, 963, 125, 125}, KeyStyle::Round},
    {"Back", "", "", "", {890, 963, 125, 125}, KeyStyle::Round},
    {"Home", "", "", "", {488, 908, 153, 98}, KeyStyle::Home},
    {"Power", "", "", "", {488, 1046, 153, 98}, KeyStyle::Power},

    {"Shift", "shift", "", "", {115, 1214, 126, 85}, KeyStyle::Small},
    {"Alpha", "alpha", "ALPHA", "", {270, 1214, 126, 85}, KeyStyle::Small},
    {"XNT", "x,n,t", "cut", ":", {425, 1214, 126, 85}, KeyStyle::Small},
    {"Var", "var", "copy", ";", {580, 1214, 126, 85}, KeyStyle::Small},
    {"Toolbox", "", "paste", "\"", {735, 1214, 126, 85}, KeyStyle::Small},
    {"Backspace", "", "clear", "%", {890, 1214, 126, 85}, KeyStyle::Small},

    {"Exp", u8"eˣ", "[", "A", {115, 1338, 126, 85}, KeyStyle::Small},
    {"Ln", "ln", "]", "B", {270, 1338, 126, 85}, KeyStyle::Small},
    {"Log", "log", "{", "C", {425, 1338, 126, 85}, KeyStyle::Small},
    {"Imaginary", "i", "}", "D", {580, 1338, 126, 85}, KeyStyle::Small},
    {"Comma", ",", u8"−", "E", {735, 1338, 126, 85}, KeyStyle::Small},
    {"PowerKey", u8"xʸ", u8"→", "F", {890, 1338, 126, 85}, KeyStyle::Small},

    {"Sin", "sin", "asin", "G", {115, 1463, 126, 85}, KeyStyle::Small},
    {"Cos", "cos", "acos", "H", {270, 1463, 126, 85}, KeyStyle::Small},
    {"Tan", "tan", "atan", "I", {425, 1463, 126, 85}, KeyStyle::Small},
    {"Pi", u8"π", "=", "J", {580, 1463, 126, 85}, KeyStyle::Small},
    {"Sqrt", u8"√", "<", "K", {735, 1463, 126, 85}, KeyStyle::Small},
    {"Square", u8"x²", ">", "L", {890, 1463, 126, 85}, KeyStyle::Small},

    {"7", "7", "", "M", {115, 1587, 153, 98}, KeyStyle::Large},
    {"8", "8", "", "N", {301, 1587, 153, 98}, KeyStyle::Large},
    {"9", "9", "", "O", {488, 1587, 153, 98}, KeyStyle::Large},
    {"LeftParenthesis", "(", "", "P", {675, 1587, 153, 98}, KeyStyle::Large},
    {"RightParenthesis", ")", "", "Q", {862, 1587, 153, 98}, KeyStyle::Large},
    {"4", "4", "", "R", {115, 1726, 153, 98}, KeyStyle::Large},
    {"5", "5", "", "S", {301, 1726, 153, 98}, KeyStyle::Large},
    {"6", "6", "", "T", {488, 1726, 153, 98}, KeyStyle::Large},
    {"Multiply", u8"×", "", "U", {675, 1726, 153, 98}, KeyStyle::Large},
    {"Divide", u8"÷", "", "V", {862, 1726, 153, 98}, KeyStyle::Large},
    {"1", "1", "", "W", {115, 1864, 153, 98}, KeyStyle::Large},
    {"2", "2", "", "X", {301, 1864, 153, 98}, KeyStyle::Large},
    {"3", "3", "", "Y", {488, 1864, 153, 98}, KeyStyle::Large},
    {"Plus", "+", "", "Z", {675, 1864, 153, 98}, KeyStyle::Large},
    {"Minus", u8"−", "", "", {862, 1864, 153, 98}, KeyStyle::Large},
    {"0", "0", "?", "", {115, 2003, 153, 98}, KeyStyle::Large},
    {"Dot", ".", "!", "", {301, 2003, 153, 98}, KeyStyle::Large},
    {"EE", u8"×10ˣ", "", "", {488, 2003, 153, 98}, KeyStyle::Large},
    {"Ans", "Ans", "", "", {675, 2003, 153, 98}, KeyStyle::Large},
    {"Equals", "EXE", "", "", {862, 2003, 153, 98}, KeyStyle::Large},
}};

static_assert(Keys.size() == 46);

int keyIndex(const QString &name) {
    for (size_t i = 0; i < Keys.size(); ++i) {
        if (name == QLatin1String(Keys[i].name)) {
            return static_cast<int>(i);
        }
    }
    return -1;
}

int keyIndexForQt(int key) {
    if (key >= Qt::Key_0 && key <= Qt::Key_9) {
        return keyIndex(QString::number(key - Qt::Key_0));
    }
    switch (key) {
    case Qt::Key_Left: return keyIndex("Left");
    case Qt::Key_Up: return keyIndex("Up");
    case Qt::Key_Down: return keyIndex("Down");
    case Qt::Key_Right: return keyIndex("Right");
    case Qt::Key_Return:
    case Qt::Key_Enter:
    case Qt::Key_Equal: return keyIndex("Equals");
    case Qt::Key_Space: return keyIndex("OK");
    case Qt::Key_Escape: return keyIndex("Back");
    case Qt::Key_Home: return keyIndex("Home");
    case Qt::Key_F1: return keyIndex("Power");
    case Qt::Key_Shift: return keyIndex("Shift");
    case Qt::Key_Alt: return keyIndex("Alpha");
    case Qt::Key_Tab: return keyIndex("Toolbox");
    case Qt::Key_Backspace: return keyIndex("Backspace");
    case Qt::Key_Comma: return keyIndex("Comma");
    case Qt::Key_Period: return keyIndex("Dot");
    case Qt::Key_ParenLeft: return keyIndex("LeftParenthesis");
    case Qt::Key_ParenRight: return keyIndex("RightParenthesis");
    case Qt::Key_Plus: return keyIndex("Plus");
    case Qt::Key_Minus: return keyIndex("Minus");
    case Qt::Key_Asterisk: return keyIndex("Multiply");
    case Qt::Key_Slash: return keyIndex("Divide");
    case Qt::Key_AsciiCircum: return keyIndex("PowerKey");
    default: return -1;
    }
}

bool decodeLedMarker(const QByteArray &bytes, QColor *color) {
    static const QRegularExpression marker(
        QStringLiteral("LCLED:([0-9A-Fa-f]+):([0-9A-Fa-f]+):([0-9A-Fa-f]+):"
                       "([0-9A-Fa-f]+):([0-9A-Fa-f]+):([0-9A-Fa-f]+):([0-9A-Fa-f]+)"));
    auto matches = marker.globalMatch(QString::fromLatin1(bytes));
    QRegularExpressionMatch match;
    while (matches.hasNext()) {
        match = matches.next();
    }
    if (!match.hasMatch()) {
        return false;
    }

    std::array<quint32, 7> values{};
    for (int i = 0; i < static_cast<int>(values.size()); ++i) {
        bool valid = false;
        values[i] = match.captured(i + 1).toUInt(&valid, 16);
        if (!valid) {
            return false;
        }
    }

    const quint32 cr1 = values[0];
    const quint32 ccer = values[1];
    const quint32 period = values[2] & 0xffff;
    const quint32 bdtr = values[6];
    if (!(cr1 & 1) || !(bdtr & 0x8000) || period == 0) {
        *color = Qt::black;
        return true;
    }

    const auto channel = [ccer, period](quint32 compare, int enableBit) {
        if (!(ccer & (1u << enableBit))) {
            return 0;
        }
        compare = qMin(compare & 0xffff, period);
        return qRound(255.0 * static_cast<qreal>(period - compare) / period);
    };
    *color = QColor(channel(values[3], 0), channel(values[4], 4), channel(values[5], 8));
    return true;
}

} // namespace

class Panel final : public QWidget {
public:
    explicit Panel(bool startEmulator = true) {
        setWindowTitle(QStringLiteral("LibreCalc N0120 emulator"));
        setAccessibleName(QStringLiteral("LibreCalc N0120 calculator emulator"));
        setFocusPolicy(Qt::StrongFocus);
        setMouseTracking(true);
        setAttribute(Qt::WA_AcceptTouchEvents);
        setMinimumSize(360, 700);
        resize(520, 1040);

        rootPath = qEnvironmentVariable("LIBRECALC_ROOT", QDir::currentPath());
        display = QImage(320, 240, QImage::Format_RGB32);
        display.fill(QColor("#f5f7f8"));

        connect(&screenRefresh, &QTimer::timeout, this, [this] { refreshScreen(); });
        connect(&ledPoll, &QTimer::timeout, this, [this] {
            if (ledQueryPending) {
                return;
            }
            ledQueryPending = true;
            sendCommand(QStringLiteral(
                "python \"b=self.Machine['sysbus']; print('LCLED:%X:%X:%X:%X:%X:%X:%X' % "
                "(b.ReadDoubleWord(0x40010000),b.ReadDoubleWord(0x40010020),"
                "b.ReadDoubleWord(0x4001002C),b.ReadDoubleWord(0x40010034),"
                "b.ReadDoubleWord(0x40010038),b.ReadDoubleWord(0x4001003C),"
                "b.ReadDoubleWord(0x40010044)))\""));
        });
        connect(&monitor, &QTcpSocket::readyRead, this, [this] {
            monitorBuffer += monitor.readAll();
            QColor decoded;
            if (decodeLedMarker(monitorBuffer, &decoded)) {
                ledColor = decoded;
                ledQueryPending = false;
                monitorBuffer.clear();
                update();
            }
            if (monitorBuffer.size() > 4096) {
                monitorBuffer = monitorBuffer.right(1024);
            }
        });

        if (startEmulator) {
            screenRefresh.start(16);
            startRenode();
        }
    }

    ~Panel() override {
        releaseAllInput();
        closing = true;
        if (monitor.state() == QAbstractSocket::ConnectedState) {
            sendCommand(QStringLiteral("runMacro $persistStorage"));
            sendCommand(QStringLiteral("quit"));
            monitor.waitForBytesWritten(1000);
            renode.waitForFinished(5000);
        }
        if (renode.state() != QProcess::NotRunning) {
            renode.kill();
            renode.waitForFinished(1000);
        }
    }

    bool selfTest(QString *failure) {
        QSet<QString> names;
        for (size_t i = 0; i < Keys.size(); ++i) {
            const QString name = QLatin1String(Keys[i].name);
            if (names.contains(name) || hitKey(Keys[i].rect.center()) != static_cast<int>(i)) {
                *failure = QStringLiteral("key geometry failed for %1").arg(name);
                return false;
            }
            names.insert(name);
        }
        if (names.size() != 46 || qAbs(ScreenRect.width() / ScreenRect.height() - 4.0 / 3.0) > 0.001) {
            *failure = QStringLiteral("key count or LCD aspect ratio failed");
            return false;
        }
        if (utilityAt(ScreenshotRect.center()) != 0 || utilityAt(FullscreenRect.center()) != 1 ||
            hitKey(ScreenshotRect.center()) != -1) {
            *failure = QStringLiteral("utility hit testing failed");
            return false;
        }
        for (const QSize size : {QSize(360, 700), QSize(800, 600), QSize(1920, 1080)}) {
            resize(size);
            const QRectF canvas = canvasTransform().mapRect(QRectF(0, 0, CanvasWidth, CanvasHeight));
            if (!rect().adjusted(-1, -1, 1, 1).contains(canvas.toAlignedRect())) {
                *failure = QStringLiteral("canvas clipping at %1x%2").arg(size.width()).arg(size.height());
                return false;
            }
        }
        if (keyIndexForQt(Qt::Key_Return) != keyIndex("Equals") ||
            keyIndexForQt(Qt::Key_Space) != keyIndex("OK") ||
            keyIndexForQt(Qt::Key_A) != -1) {
            *failure = QStringLiteral("physical keyboard mapping failed");
            return false;
        }

        QColor decoded(Qt::magenta);
        if (!decodeLedMarker("LCLED:1:111:3A:0:3A:3A:8000", &decoded) || decoded != QColor(255, 0, 0) ||
            decodeLedMarker("LCLED:not-a-register-dump", &decoded) ||
            !decodeLedMarker("LCLED:0:111:3A:0:0:0:8000", &decoded) || decoded != Qt::black) {
            *failure = QStringLiteral("LED monitor decoding failed");
            return false;
        }

        QPainter testPainter(&display);
        testPainter.fillRect(display.rect(), Qt::white);
        testPainter.fillRect(0, 0, display.width(), 28, QColor("#ffb531"));
        testPainter.setPen(QColor("#202428"));
        testPainter.setFont(QFont(QStringLiteral("Sans Serif"), 14, QFont::DemiBold));
        testPainter.drawText(display.rect(), Qt::AlignCenter, QStringLiteral("LibreCalc UI self-test"));
        testPainter.end();
        hasDisplay = true;
        ledColor = QColor("#ff9f1a");
        held[keyIndex("6")] = 1;
        held[keyIndex("OK")] = 1;
        resize(520, 1040);
        show();
        QApplication::processEvents();

        QImage preview(size(), QImage::Format_ARGB32_Premultiplied);
        preview.fill(Qt::transparent);
        QPainter previewPainter(&preview);
        render(&previewPainter);
        previewPainter.end();
        QDir().mkpath(rootPath + QStringLiteral("/build"));
        const QString path = rootPath + QStringLiteral("/build/panel-preview.png");
        if (!preview.save(path)) {
            *failure = QStringLiteral("could not write %1").arg(path);
            return false;
        }
        return true;
    }

protected:
    void paintEvent(QPaintEvent *) override {
        QPainter painter(this);
        painter.setRenderHint(QPainter::Antialiasing);
        painter.setRenderHint(QPainter::TextAntialiasing);
        painter.fillRect(rect(), QColor("#dfe3e6"));
        painter.setTransform(canvasTransform());

        const QRectF body(51, 18, 1028, 2172);
        painter.setPen(Qt::NoPen);
        painter.setBrush(QColor(0, 0, 0, 42));
        painter.drawRoundedRect(body.translated(0, 16), 64, 64);
        QLinearGradient bodyGradient(body.topLeft(), body.bottomRight());
        bodyGradient.setColorAt(0, QColor("#ffffff"));
        bodyGradient.setColorAt(0.55, QColor("#f7f7f5"));
        bodyGradient.setColorAt(1, QColor("#ececea"));
        painter.setBrush(bodyGradient);
        painter.setPen(QPen(QColor("#d5d5d2"), 2));
        painter.drawRoundedRect(body, 64, 64);

        painter.setPen(QColor("#24272a"));
        QFont brandFont(QStringLiteral("Sans Serif"));
        brandFont.setPixelSize(36);
        brandFont.setWeight(QFont::Medium);
        brandFont.setLetterSpacing(QFont::AbsoluteSpacing, 5);
        painter.setFont(brandFont);
        painter.drawText(QRectF(250, 74, 630, 54), Qt::AlignCenter, QStringLiteral("LIBRECALC"));

        if (ledColor != Qt::black) {
            QRadialGradient glow(QPointF(565, 35), 62);
            QColor halo = ledColor;
            halo.setAlpha(115);
            glow.setColorAt(0, halo);
            halo.setAlpha(0);
            glow.setColorAt(1, halo);
            painter.setPen(Qt::NoPen);
            painter.setBrush(glow);
            painter.drawEllipse(QPointF(565, 35), 62, 35);
        }
        painter.setPen(QPen(QColor("#858585"), 2));
        painter.setBrush(ledColor == Qt::black ? QColor("#343638") : ledColor.lighter(125));
        painter.drawRoundedRect(QRectF(542, 25, 46, 14), 7, 7);

        const QRectF bezel = ScreenRect.adjusted(-23, -23, 23, 23);
        QLinearGradient bezelGradient(bezel.topLeft(), bezel.bottomLeft());
        bezelGradient.setColorAt(0, QColor("#b8b8b6"));
        bezelGradient.setColorAt(0.12, QColor("#efefed"));
        bezelGradient.setColorAt(1, QColor("#ffffff"));
        painter.setBrush(bezelGradient);
        painter.setPen(QPen(QColor("#dededb"), 2));
        painter.drawRoundedRect(bezel, 42, 42);
        painter.setPen(Qt::NoPen);
        painter.setBrush(QColor("#252729"));
        painter.drawRoundedRect(ScreenRect.adjusted(-5, -5, 5, 5), 7, 7);
        painter.setRenderHint(QPainter::SmoothPixmapTransform, false);
        painter.drawImage(ScreenRect, display, display.rect());

        if (!hasDisplay) {
            painter.fillRect(ScreenRect, QColor("#f4f7f8"));
            painter.setPen(QColor("#3d454b"));
            QFont statusFont(QStringLiteral("Sans Serif"));
            statusFont.setPixelSize(32);
            painter.setFont(statusFont);
            painter.drawText(ScreenRect.adjusted(70, 70, -70, -70),
                             Qt::AlignCenter | Qt::TextWordWrap, statusText);
        } else if (statusIsError && !statusText.isEmpty()) {
            const QRectF errorRect(ScreenRect.left(), ScreenRect.bottom() - 82, ScreenRect.width(), 82);
            painter.fillRect(errorRect, QColor(24, 27, 29, 220));
            painter.setPen(Qt::white);
            QFont errorFont(QStringLiteral("Sans Serif"));
            errorFont.setPixelSize(25);
            painter.setFont(errorFont);
            painter.drawText(errorRect.adjusted(20, 4, -20, -4), Qt::AlignCenter, statusText);
        }

        QPainterPath vertical;
        vertical.addRoundedRect(QRectF(190, 884, 129, 280), 46, 46);
        QPainterPath horizontal;
        horizontal.addRoundedRect(QRectF(116, 960, 281, 129), 46, 46);
        const QPainterPath dpad = vertical.united(horizontal);
        painter.setPen(Qt::NoPen);
        painter.setBrush(QColor(0, 0, 0, 45));
        painter.drawPath(dpad.translated(0, 8));
        QLinearGradient dpadGradient(0, 884, 0, 1164);
        dpadGradient.setColorAt(0, QColor("#ffffff"));
        dpadGradient.setColorAt(1, QColor("#e5e5e3"));
        painter.setBrush(dpadGradient);
        painter.setPen(QPen(QColor("#55585a"), 2));
        painter.drawPath(dpad);
        painter.save();
        painter.setClipPath(dpad);
        for (size_t i = 0; i < 4; ++i) {
            if (held.value(static_cast<int>(i), 0) > 0) {
                painter.fillRect(Keys[i].rect, QColor("#d7d8d6"));
            }
        }
        painter.restore();
        for (size_t i = 0; i < 4; ++i) {
            drawArrow(painter, Keys[i].rect.center(), static_cast<int>(i));
        }

        for (size_t i = 4; i < Keys.size(); ++i) {
            drawKey(painter, static_cast<int>(i));
        }

        drawUtility(painter, ScreenshotRect, 0, utilityDown == 0, utilityHover == 0);
        drawUtility(painter, FullscreenRect, 1, utilityDown == 1, utilityHover == 1);
    }

    void mousePressEvent(QMouseEvent *event) override {
        if (event->button() != Qt::LeftButton) {
            return QWidget::mousePressEvent(event);
        }
        const QPointF point = toCanvas(event->position());
        utilityDown = utilityAt(point);
        if (utilityDown < 0) {
            mouseKey = hitKey(point);
            pressKey(mouseKey);
        }
        update();
        event->accept();
    }

    void mouseMoveEvent(QMouseEvent *event) override {
        const QPointF point = toCanvas(event->position());
        utilityHover = utilityAt(point);
        if (event->buttons() & Qt::LeftButton && utilityDown < 0) {
            const int next = hitKey(point);
            if (next != mouseKey) {
                releaseKey(mouseKey);
                mouseKey = next;
                pressKey(mouseKey);
            }
        }
        const int hoverKey = hitKey(point);
        if (utilityHover == 0) {
            setToolTip(QStringLiteral("Save LCD screenshot (Ctrl+S)"));
        } else if (utilityHover == 1) {
            setToolTip(QStringLiteral("Toggle fullscreen (F11)"));
        } else if (hoverKey >= 0) {
            setToolTip(QLatin1String(Keys[hoverKey].name));
        } else {
            setToolTip(QString());
        }
        setCursor(utilityHover >= 0 || hoverKey >= 0 ? Qt::PointingHandCursor : Qt::ArrowCursor);
        update();
        event->accept();
    }

    void mouseReleaseEvent(QMouseEvent *event) override {
        if (event->button() != Qt::LeftButton) {
            return QWidget::mouseReleaseEvent(event);
        }
        const int action = utilityDown;
        const bool activate = action >= 0 && action == utilityAt(toCanvas(event->position()));
        utilityDown = -1;
        releaseKey(mouseKey);
        mouseKey = -1;
        update();
        if (activate) {
            action == 0 ? saveScreenshot() : toggleFullscreen();
        }
        event->accept();
    }

    void keyPressEvent(QKeyEvent *event) override {
        if (event->isAutoRepeat()) {
            event->accept();
            return;
        }
        if (event->key() == Qt::Key_F11) {
            toggleFullscreen();
            event->accept();
            return;
        }
        if (event->key() == Qt::Key_Escape && isFullScreen()) {
            showNormal();
            event->accept();
            return;
        }
        if (event->matches(QKeySequence::Save)) {
            saveScreenshot();
            event->accept();
            return;
        }
        const int index = keyIndexForQt(event->key());
        if (index >= 0) {
            if (!keyboardKeys.contains(event->key())) {
                keyboardKeys[event->key()] = index;
                pressKey(index);
            }
            event->accept();
            return;
        }
        QWidget::keyPressEvent(event);
    }

    void keyReleaseEvent(QKeyEvent *event) override {
        if (event->isAutoRepeat()) {
            event->accept();
            return;
        }
        const auto found = keyboardKeys.find(event->key());
        if (found != keyboardKeys.end()) {
            const int index = found.value();
            keyboardKeys.erase(found);
            releaseKey(index);
            event->accept();
            return;
        }
        QWidget::keyReleaseEvent(event);
    }

    bool event(QEvent *event) override {
        if (event->type() == QEvent::TouchBegin || event->type() == QEvent::TouchUpdate ||
            event->type() == QEvent::TouchEnd) {
            auto *touch = static_cast<QTouchEvent *>(event);
            for (const QEventPoint &point : touch->points()) {
                const int id = point.id();
                if (point.state() == QEventPoint::State::Pressed) {
                    const int index = hitKey(toCanvas(point.position()));
                    if (index >= 0) {
                        touchKeys[id] = index;
                        pressKey(index);
                    }
                } else if (point.state() == QEventPoint::State::Updated) {
                    const int next = hitKey(toCanvas(point.position()));
                    const int previous = touchKeys.value(id, -1);
                    if (next != previous) {
                        releaseKey(previous);
                        touchKeys.remove(id);
                        if (next >= 0) {
                            touchKeys[id] = next;
                            pressKey(next);
                        }
                    }
                } else if (point.state() == QEventPoint::State::Released) {
                    if (touchKeys.contains(id)) {
                        releaseKey(touchKeys.take(id));
                    }
                }
            }
            if (event->type() == QEvent::TouchEnd) {
                const QList<int> activeTouches = touchKeys.values();
                touchKeys.clear();
                for (int index : activeTouches) {
                    releaseKey(index);
                }
            }
            event->accept();
            return true;
        }
        if (event->type() == QEvent::TouchCancel || event->type() == QEvent::WindowDeactivate) {
            releaseAllInput();
        }
        return QWidget::event(event);
    }

private:
    QProcess renode;
    QTcpSocket monitor;
    QTimer connector;
    QTimer screenRefresh;
    QTimer ledPoll;
    QImage display;
    QColor ledColor = Qt::black;
    QByteArray monitorBuffer;
    QHash<int, int> held;
    QHash<int, int> keyboardKeys;
    QHash<int, int> touchKeys;
    QString rootPath;
    QString statusText = QStringLiteral("Starting calculator…");
    quint16 port = 0;
    qint64 screenTimestamp = -1;
    int mouseKey = -1;
    int utilityDown = -1;
    int utilityHover = -1;
    bool hasDisplay = false;
    bool statusIsError = false;
    bool closing = false;
    bool ledQueryPending = false;

    QTransform canvasTransform() const {
        const qreal margin = 12.0;
        const qreal scale = qMax<qreal>(0.01, qMin((width() - 2 * margin) / CanvasWidth,
                                                  (height() - 2 * margin) / CanvasHeight));
        QTransform transform;
        transform.translate((width() - CanvasWidth * scale) / 2.0,
                            (height() - CanvasHeight * scale) / 2.0);
        transform.scale(scale, scale);
        return transform;
    }

    QPointF toCanvas(const QPointF &point) const {
        return canvasTransform().inverted().map(point);
    }

    int hitKey(const QPointF &point) const {
        for (size_t i = 0; i < Keys.size(); ++i) {
            if (Keys[i].rect.contains(point)) {
                return static_cast<int>(i);
            }
        }
        return -1;
    }

    static int utilityAt(const QPointF &point) {
        if (ScreenshotRect.contains(point)) {
            return 0;
        }
        if (FullscreenRect.contains(point)) {
            return 1;
        }
        return -1;
    }

    void drawArrow(QPainter &painter, const QPointF &center, int direction) const {
        QPainterPath arrow;
        arrow.moveTo(0, -17);
        arrow.lineTo(18, 14);
        arrow.lineTo(-18, 14);
        arrow.closeSubpath();
        painter.save();
        painter.translate(center);
        painter.rotate(direction == 0 ? -90 : direction == 2 ? 180 : direction == 3 ? 90 : 0);
        painter.setPen(QPen(QColor("#272a2c"), 5, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
        painter.setBrush(Qt::NoBrush);
        painter.drawPath(arrow);
        painter.restore();
    }

    void drawKey(QPainter &painter, int index) const {
        const KeyDefinition &key = Keys[index];
        const bool pressed = held.value(index, 0) > 0;
        QRectF keyRect = key.rect.translated(0, pressed ? 5 : 0);
        QPainterPath shape;
        if (key.style == KeyStyle::Round) {
            shape.addEllipse(keyRect);
        } else {
            shape.addRoundedRect(keyRect, key.style == KeyStyle::Small ? 34 : 43,
                                 key.style == KeyStyle::Small ? 34 : 43);
        }

        QPainterPath shadow;
        const QRectF shadowRect = key.rect.translated(0, pressed ? 7 : 10);
        if (key.style == KeyStyle::Round) {
            shadow.addEllipse(shadowRect);
        } else {
            shadow.addRoundedRect(shadowRect, key.style == KeyStyle::Small ? 34 : 43,
                                  key.style == KeyStyle::Small ? 34 : 43);
        }
        painter.setPen(Qt::NoPen);
        painter.setBrush(QColor(0, 0, 0, pressed ? 32 : 55));
        painter.drawPath(shadow);

        QColor top("#ffffff");
        QColor bottom(pressed ? "#d7d8d6" : "#e9e9e7");
        if (key.style == KeyStyle::Home) {
            top = pressed ? QColor("#dc9826") : QColor("#ffc552");
            bottom = pressed ? QColor("#cf8d20") : QColor("#efa92f");
        } else if (key.style == KeyStyle::Power) {
            top = pressed ? QColor("#1f2021") : QColor("#3b3c3d");
            bottom = QColor("#252627");
        }
        QLinearGradient gradient(keyRect.topLeft(), keyRect.bottomLeft());
        gradient.setColorAt(0, top);
        gradient.setColorAt(1, bottom);
        painter.setBrush(gradient);
        painter.setPen(QPen(QColor("#55585a"), 2));
        painter.drawPath(shape);

        painter.save();
        painter.translate(0, pressed ? 5 : 0);
        if (key.style == KeyStyle::Home) {
            const QPointF c = key.rect.center();
            QPainterPath home;
            home.moveTo(c.x() - 25, c.y() + 3);
            home.lineTo(c.x(), c.y() - 20);
            home.lineTo(c.x() + 25, c.y() + 3);
            home.moveTo(c.x() - 18, c.y() - 2);
            home.lineTo(c.x() - 18, c.y() + 22);
            home.lineTo(c.x() + 18, c.y() + 22);
            home.lineTo(c.x() + 18, c.y() - 2);
            painter.setPen(QPen(Qt::white, 7, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
            painter.setBrush(Qt::NoBrush);
            painter.drawPath(home);
        } else if (key.style == KeyStyle::Power) {
            const QPointF c = key.rect.center();
            painter.setPen(QPen(Qt::white, 7, Qt::SolidLine, Qt::RoundCap));
            painter.setBrush(Qt::NoBrush);
            painter.drawArc(QRectF(c.x() - 23, c.y() - 21, 46, 46), 45 * 16, 270 * 16);
            painter.drawLine(QPointF(c.x(), c.y() - 28), QPointF(c.x(), c.y() - 2));
        } else if (qstrcmp(key.name, "Back") == 0) {
            const QPointF c = key.rect.center();
            QPainterPath back;
            back.moveTo(c.x() + 28, c.y() - 17);
            back.lineTo(c.x() - 12, c.y() - 17);
            back.lineTo(c.x() - 12, c.y() - 31);
            back.lineTo(c.x() - 37, c.y());
            back.lineTo(c.x() - 12, c.y() + 31);
            back.lineTo(c.x() - 12, c.y() + 17);
            back.lineTo(c.x() + 23, c.y() + 17);
            painter.setPen(QPen(QColor("#25282a"), 7, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
            painter.setBrush(Qt::NoBrush);
            painter.drawPath(back);
        } else if (qstrcmp(key.name, "Toolbox") == 0) {
            const QPointF c = key.rect.center();
            painter.setPen(QPen(QColor("#25282a"), 4, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
            painter.setBrush(Qt::NoBrush);
            painter.drawRoundedRect(QRectF(c.x() - 24, c.y() - 4, 48, 28), 4, 4);
            painter.drawRect(QRectF(c.x() - 10, c.y() - 14, 20, 10));
            painter.drawLine(QPointF(c.x() - 24, c.y() + 7), QPointF(c.x() + 24, c.y() + 7));
        } else if (qstrcmp(key.name, "Backspace") == 0) {
            const QPointF c = key.rect.center();
            QPainterPath erase;
            erase.moveTo(c.x() - 29, c.y());
            erase.lineTo(c.x() - 12, c.y() - 18);
            erase.lineTo(c.x() + 29, c.y() - 18);
            erase.lineTo(c.x() + 29, c.y() + 18);
            erase.lineTo(c.x() - 12, c.y() + 18);
            erase.closeSubpath();
            painter.setPen(QPen(QColor("#25282a"), 4, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
            painter.setBrush(Qt::NoBrush);
            painter.drawPath(erase);
            painter.drawLine(QPointF(c.x() - 1, c.y() - 9), QPointF(c.x() + 16, c.y() + 9));
            painter.drawLine(QPointF(c.x() + 16, c.y() - 9), QPointF(c.x() - 1, c.y() + 9));
        } else {
            const bool large = key.style == KeyStyle::Large || key.style == KeyStyle::Round;
            QFont mainFont(QStringLiteral("Sans Serif"));
            mainFont.setPixelSize(large ? 43 : 29);
            mainFont.setWeight(QFont::Medium);
            painter.setFont(mainFont);
            painter.setPen(qstrcmp(key.name, "Shift") == 0 ? QColor("#e49a13") : QColor("#17191a"));
            painter.drawText(key.rect.adjusted(3, large ? 10 : 19, -3, -2),
                             Qt::AlignCenter, QString::fromUtf8(key.main));
        }

        if (*key.orange) {
            QFont secondary(QStringLiteral("Sans Serif"));
            secondary.setPixelSize(21);
            secondary.setWeight(QFont::DemiBold);
            painter.setFont(secondary);
            painter.setPen(QColor("#e49a13"));
            painter.drawText(key.rect.adjusted(10, 3, -8, -3), Qt::AlignLeft | Qt::AlignTop,
                             QString::fromUtf8(key.orange));
        }
        if (*key.alpha) {
            QFont alpha(QStringLiteral("Sans Serif"));
            alpha.setPixelSize(21);
            painter.setFont(alpha);
            painter.setPen(QColor("#5b6063"));
            painter.drawText(key.rect.adjusted(8, 3, -10, -3), Qt::AlignRight | Qt::AlignTop,
                             QString::fromUtf8(key.alpha));
        }
        painter.restore();
    }

    void drawUtility(QPainter &painter, const QRectF &button, int action, bool down, bool hover) const {
        const QRectF drawn = button.translated(0, down ? 4 : 0);
        painter.setPen(QPen(QColor("#757b7f"), 2));
        painter.setBrush(hover ? QColor("#ffffff") : QColor("#f2f4f5"));
        painter.drawEllipse(drawn);
        painter.save();
        painter.translate(0, down ? 4 : 0);
        painter.setPen(QPen(QColor("#454a4e"), 5, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
        painter.setBrush(Qt::NoBrush);
        if (action == 0) {
            const QPointF c = button.center();
            painter.drawRoundedRect(QRectF(c.x() - 25, c.y() - 17, 50, 36), 5, 5);
            painter.drawEllipse(QRectF(c.x() - 9, c.y() - 10, 18, 18));
            painter.drawLine(QPointF(c.x() - 13, c.y() - 17), QPointF(c.x() - 5, c.y() - 25));
            painter.drawLine(QPointF(c.x() - 5, c.y() - 25), QPointF(c.x() + 7, c.y() - 25));
        } else {
            const QRectF r = button.adjusted(22, 22, -22, -22);
            painter.drawLine(r.topLeft(), QPointF(r.left() + 14, r.top()));
            painter.drawLine(r.topLeft(), QPointF(r.left(), r.top() + 14));
            painter.drawLine(r.topRight(), QPointF(r.right() - 14, r.top()));
            painter.drawLine(r.topRight(), QPointF(r.right(), r.top() + 14));
            painter.drawLine(r.bottomLeft(), QPointF(r.left() + 14, r.bottom()));
            painter.drawLine(r.bottomLeft(), QPointF(r.left(), r.bottom() - 14));
            painter.drawLine(r.bottomRight(), QPointF(r.right() - 14, r.bottom()));
            painter.drawLine(r.bottomRight(), QPointF(r.right(), r.bottom() - 14));
        }
        painter.restore();
    }

    void pressKey(int index) {
        if (index < 0) {
            return;
        }
        const int count = held.value(index, 0);
        held[index] = count + 1;
        if (count == 0) {
            sendCommand(QStringLiteral("gpioPortA.n0120Keypad Press \"%1\"")
                            .arg(QLatin1String(Keys[index].name)));
        }
        update();
    }

    void releaseKey(int index) {
        if (index < 0 || !held.contains(index)) {
            return;
        }
        const int count = held.value(index) - 1;
        if (count > 0) {
            held[index] = count;
        } else {
            held.remove(index);
            sendCommand(QStringLiteral("gpioPortA.n0120Keypad Release \"%1\"")
                            .arg(QLatin1String(Keys[index].name)));
        }
        update();
    }

    void releaseAllInput() {
        const QList<int> active = held.keys();
        held.clear();
        keyboardKeys.clear();
        touchKeys.clear();
        mouseKey = -1;
        for (int index : active) {
            sendCommand(QStringLiteral("gpioPortA.n0120Keypad Release \"%1\"")
                            .arg(QLatin1String(Keys[index].name)));
        }
        update();
    }

    void saveScreenshot() {
        if (!hasDisplay) {
            setStatus(QStringLiteral("No LCD frame to save yet"), true);
            return;
        }
        QString path = QFileDialog::getSaveFileName(
            this, QStringLiteral("Save LCD screenshot"),
            QDir::homePath() + QStringLiteral("/librecalc-screen.png"),
            QStringLiteral("PNG image (*.png)"));
        if (path.isEmpty()) {
            return;
        }
        if (!path.endsWith(QStringLiteral(".png"), Qt::CaseInsensitive)) {
            path += QStringLiteral(".png");
        }
        if (!display.save(path, "PNG")) {
            setStatus(QStringLiteral("Could not save screenshot"), true);
        }
    }

    void toggleFullscreen() {
        isFullScreen() ? showNormal() : showFullScreen();
    }

    void refreshScreen() {
        const QString path = rootPath + QStringLiteral("/emulator/state/lcd.ppm");
        const QFileInfo info(path);
        if (!info.exists()) {
            return;
        }
        const qint64 modified = info.lastModified().toMSecsSinceEpoch();
        if (modified == screenTimestamp) {
            return;
        }
        QImage next(path);
        if (next.size() == QSize(320, 240)) {
            display = next.convertToFormat(QImage::Format_RGB32);
            screenTimestamp = modified;
            hasDisplay = true;
            if (!statusIsError) {
                statusText.clear();
            }
            update();
        }
    }

    void setStatus(const QString &message, bool error) {
        statusText = message;
        statusIsError = error;
        update();
    }

    void startRenode() {
        if (!QFileInfo::exists(rootPath + QStringLiteral("/emulator/state/internal.bin")) ||
            !QFileInfo::exists(rootPath + QStringLiteral("/emulator/state/external.bin"))) {
            setStatus(QStringLiteral("Flash virtual storage first\n\nmake flash-virtual IMAGE=firmware.dfu"), true);
            return;
        }

        QTcpServer probe;
        if (!probe.listen(QHostAddress::LocalHost, 0)) {
            setStatus(QStringLiteral("Could not allocate the Renode monitor port"), true);
            return;
        }
        port = probe.serverPort();
        probe.close();

        renode.setWorkingDirectory(rootPath);
        renode.setStandardOutputFile(QProcess::nullDevice());
        renode.setStandardErrorFile(QProcess::nullDevice());
        connect(&renode, &QProcess::errorOccurred, this, [this] {
            setStatus(QStringLiteral("Renode failed to start\nCheck the RENODE command"), true);
        });
        connect(&renode, &QProcess::finished, this, [this] {
            if (!closing) {
                setStatus(QStringLiteral("Renode stopped"), true);
            }
        });
        renode.start(qEnvironmentVariable("RENODE", "renode"),
                     {QStringLiteral("--disable-xwt"), QStringLiteral("--hide-monitor"),
                      QStringLiteral("-P"), QString::number(port)});

        connect(&connector, &QTimer::timeout, this, [this] {
            if (monitor.state() == QAbstractSocket::UnconnectedState) {
                monitor.connectToHost(QHostAddress::LocalHost, port);
            }
        });
        connect(&monitor, &QTcpSocket::connected, this, [this] {
            connector.stop();
            QTimer::singleShot(300, this, [this] {
                sendCommand(QStringLiteral("include @emulator/run.resc"));
                setStatus(QStringLiteral("Waiting for the LCD…"), false);
                ledPoll.start(100);
            });
        });
        connector.start(100);
    }

    void sendCommand(const QString &command) {
        if (monitor.state() == QAbstractSocket::ConnectedState) {
            monitor.write(command.toUtf8() + "\r\n");
        }
    }
};

int main(int argc, char **argv) {
    QApplication application(argc, argv);
    QApplication::setApplicationName(QStringLiteral("LibreCalc N0120 emulator"));
    const bool selfTest = application.arguments().contains(QStringLiteral("--self-test"));
    Panel panel(!selfTest);
    if (selfTest) {
        QString failure;
        if (!panel.selfTest(&failure)) {
            std::cerr << "PANEL_SELF_TEST_FAILED: " << failure.toStdString() << '\n';
            return 1;
        }
        std::cout << "PANEL_SELF_TEST_OK build/panel-preview.png\n";
        return 0;
    }
    panel.show();
    return application.exec();
}
