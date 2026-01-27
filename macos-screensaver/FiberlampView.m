#import "FiberlampView.h"

// FFI declarations for Rust library
typedef void* FiberlampHandle;

typedef struct {
    uint32_t fiber_count;
    uint32_t ncolors;
    uint32_t msaa_samples;
} FiberlampConfig;

extern FiberlampHandle fiberlamp_init(void* ns_view, uint32_t width, uint32_t height);
extern FiberlampHandle fiberlamp_init_with_config(void* ns_view, uint32_t width, uint32_t height, FiberlampConfig config);
extern int32_t fiberlamp_animate(FiberlampHandle handle);
extern void fiberlamp_resize(FiberlampHandle handle, uint32_t width, uint32_t height);
extern void fiberlamp_destroy(FiberlampHandle handle);
extern double fiberlamp_animation_interval(void);
extern FiberlampConfig fiberlamp_default_config(void);
extern uint32_t fiberlamp_min_fibers(void);
extern uint32_t fiberlamp_max_fibers(void);

// NSUserDefaults keys
static NSString * const kFiberlampFiberCount = @"FiberlampFiberCount";
static NSString * const kFiberlampNColors = @"FiberlampNColors";
static NSString * const kFiberlampMSAA = @"FiberlampMSAA";

// Valid ncolors values (powers of 2)
static const uint32_t kValidNColors[] = {8, 16, 32, 64, 128, 256};
static const NSInteger kValidNColorsCount = 6;

// Valid MSAA values
static const uint32_t kValidMSAA[] = {1, 2, 4, 8};
static const NSInteger kValidMSAACount = 4;

@implementation FiberlampView {
    FiberlampHandle _handle;
    FiberlampConfig _config;

    // Configuration sheet UI elements
    NSSlider *_fiberSlider;
    NSTextField *_fiberLabel;
    NSSegmentedControl *_colorSegment;
    NSSegmentedControl *_msaaSegment;
}

- (instancetype)initWithFrame:(NSRect)frame isPreview:(BOOL)isPreview {
    self = [super initWithFrame:frame isPreview:isPreview];
    if (self) {
        _handle = NULL;
        _config = fiberlamp_default_config();
        // Enable layer-backed view for Metal rendering
        [self setWantsLayer:YES];
        // Set animation interval from Rust
        [self setAnimationTimeInterval:fiberlamp_animation_interval()];
    }
    return self;
}

- (ScreenSaverDefaults *)defaults {
    return [ScreenSaverDefaults defaultsForModuleWithName:@"com.jtcressy.Fiberlamp"];
}

- (BOOL)isValidNColors:(uint32_t)value {
    for (NSInteger i = 0; i < kValidNColorsCount; i++) {
        if (kValidNColors[i] == value) return YES;
    }
    return NO;
}

- (BOOL)isValidMSAA:(uint32_t)value {
    for (NSInteger i = 0; i < kValidMSAACount; i++) {
        if (kValidMSAA[i] == value) return YES;
    }
    return NO;
}

- (NSInteger)indexForNColors:(uint32_t)value {
    for (NSInteger i = 0; i < kValidNColorsCount; i++) {
        if (kValidNColors[i] == value) return i;
    }
    return 3; // Default to 64 (index 3)
}

- (NSInteger)indexForMSAA:(uint32_t)value {
    for (NSInteger i = 0; i < kValidMSAACount; i++) {
        if (kValidMSAA[i] == value) return i;
    }
    return 2; // Default to 4 (index 2)
}

- (void)loadConfig {
    ScreenSaverDefaults *defaults = [self defaults];
    FiberlampConfig defaultConfig = fiberlamp_default_config();
    uint32_t minFibers = fiberlamp_min_fibers();
    uint32_t maxFibers = fiberlamp_max_fibers();

    // Load fiber count with clamping
    uint32_t fiberCount = (uint32_t)[defaults integerForKey:kFiberlampFiberCount];
    if (fiberCount == 0) {
        fiberCount = defaultConfig.fiber_count;
    }
    if (fiberCount < minFibers) fiberCount = minFibers;
    if (fiberCount > maxFibers) fiberCount = maxFibers;
    _config.fiber_count = fiberCount;

    // Load ncolors with validation
    uint32_t ncolors = (uint32_t)[defaults integerForKey:kFiberlampNColors];
    if (![self isValidNColors:ncolors]) {
        ncolors = defaultConfig.ncolors;
    }
    _config.ncolors = ncolors;

    // Load MSAA with validation
    uint32_t msaa = (uint32_t)[defaults integerForKey:kFiberlampMSAA];
    if (![self isValidMSAA:msaa]) {
        msaa = defaultConfig.msaa_samples;
    }
    _config.msaa_samples = msaa;

    NSLog(@"Fiberlamp: Loaded config - fibers=%u, colors=%u, msaa=%u",
          _config.fiber_count, _config.ncolors, _config.msaa_samples);
}

- (void)startAnimation {
    [super startAnimation];

    if (_handle == NULL) {
        // Load configuration from user defaults
        [self loadConfig];

        // In preview mode, reduce complexity for responsiveness
        FiberlampConfig config = _config;
        if ([self isPreview]) {
            if (config.fiber_count > 100) {
                config.fiber_count = 100;
            }
            config.msaa_samples = 1; // Disable MSAA in preview
            NSLog(@"Fiberlamp: Preview mode - capped fibers=%u, msaa=%u",
                  config.fiber_count, config.msaa_samples);
        }

        NSRect bounds = [self bounds];
        CGFloat scale = [[self window] backingScaleFactor];
        if (scale < 1.0) scale = 1.0;

        uint32_t width = (uint32_t)(bounds.size.width * scale);
        uint32_t height = (uint32_t)(bounds.size.height * scale);

        // Initialize Rust renderer with configuration
        _handle = fiberlamp_init_with_config((__bridge void*)self, width, height, config);

        if (_handle == NULL) {
            NSLog(@"Fiberlamp: Failed to initialize renderer");
        }
    }
}

- (void)stopAnimation {
    [super stopAnimation];

    if (_handle != NULL) {
        fiberlamp_destroy(_handle);
        _handle = NULL;
    }
}

- (void)animateOneFrame {
    if (_handle != NULL) {
        int result = fiberlamp_animate(_handle);
        if (result == 1) {
            // Surface lost, trigger resize
            NSRect bounds = [self bounds];
            CGFloat scale = [[self window] backingScaleFactor];
            if (scale < 1.0) scale = 1.0;

            uint32_t width = (uint32_t)(bounds.size.width * scale);
            uint32_t height = (uint32_t)(bounds.size.height * scale);
            fiberlamp_resize(_handle, width, height);
        } else if (result < 0) {
            NSLog(@"Fiberlamp: Animation error %d", result);
        }
    }

    [self setNeedsDisplay:YES];
}

- (void)setFrameSize:(NSSize)newSize {
    [super setFrameSize:newSize];

    if (_handle != NULL) {
        CGFloat scale = [[self window] backingScaleFactor];
        if (scale < 1.0) scale = 1.0;

        uint32_t width = (uint32_t)(newSize.width * scale);
        uint32_t height = (uint32_t)(newSize.height * scale);
        fiberlamp_resize(_handle, width, height);
    }
}

- (void)dealloc {
    if (_handle != NULL) {
        fiberlamp_destroy(_handle);
        _handle = NULL;
    }
}

- (BOOL)hasConfigureSheet {
    return YES;
}

- (NSWindow*)configureSheet {
    if (self.configSheet) {
        return self.configSheet;
    }

    // Load current config
    [self loadConfig];

    // Create the configuration window
    NSRect windowRect = NSMakeRect(0, 0, 400, 220);
    self.configSheet = [[NSWindow alloc] initWithContentRect:windowRect
                                                   styleMask:NSWindowStyleMaskTitled
                                                     backing:NSBackingStoreBuffered
                                                       defer:YES];
    [self.configSheet setTitle:@"Fiberlamp Options"];

    NSView *contentView = [self.configSheet contentView];
    CGFloat margin = 20;
    CGFloat labelWidth = 100;
    CGFloat controlX = margin + labelWidth + 10;
    CGFloat controlWidth = windowRect.size.width - controlX - margin;
    CGFloat rowHeight = 30;
    CGFloat y = windowRect.size.height - margin - rowHeight;

    // Fiber Count row
    NSTextField *fiberTitleLabel = [[NSTextField alloc] initWithFrame:NSMakeRect(margin, y, labelWidth, 20)];
    [fiberTitleLabel setStringValue:@"Fiber Count:"];
    [fiberTitleLabel setBezeled:NO];
    [fiberTitleLabel setDrawsBackground:NO];
    [fiberTitleLabel setEditable:NO];
    [fiberTitleLabel setSelectable:NO];
    [fiberTitleLabel setAlignment:NSTextAlignmentRight];
    [contentView addSubview:fiberTitleLabel];

    _fiberSlider = [[NSSlider alloc] initWithFrame:NSMakeRect(controlX, y, controlWidth - 50, 20)];
    [_fiberSlider setMinValue:fiberlamp_min_fibers()];
    [_fiberSlider setMaxValue:fiberlamp_max_fibers()];
    [_fiberSlider setIntegerValue:_config.fiber_count];
    [_fiberSlider setTarget:self];
    [_fiberSlider setAction:@selector(fiberSliderChanged:)];
    [_fiberSlider setContinuous:YES];
    [contentView addSubview:_fiberSlider];

    _fiberLabel = [[NSTextField alloc] initWithFrame:NSMakeRect(controlX + controlWidth - 45, y, 45, 20)];
    [_fiberLabel setStringValue:[NSString stringWithFormat:@"%u", _config.fiber_count]];
    [_fiberLabel setBezeled:NO];
    [_fiberLabel setDrawsBackground:NO];
    [_fiberLabel setEditable:NO];
    [_fiberLabel setSelectable:NO];
    [_fiberLabel setAlignment:NSTextAlignmentRight];
    [contentView addSubview:_fiberLabel];

    y -= rowHeight + 10;

    // Color Palette row
    NSTextField *colorLabel = [[NSTextField alloc] initWithFrame:NSMakeRect(margin, y, labelWidth, 20)];
    [colorLabel setStringValue:@"Color Palette:"];
    [colorLabel setBezeled:NO];
    [colorLabel setDrawsBackground:NO];
    [colorLabel setEditable:NO];
    [colorLabel setSelectable:NO];
    [colorLabel setAlignment:NSTextAlignmentRight];
    [contentView addSubview:colorLabel];

    _colorSegment = [[NSSegmentedControl alloc] initWithFrame:NSMakeRect(controlX, y, controlWidth, 24)];
    [_colorSegment setSegmentCount:kValidNColorsCount];
    for (NSInteger i = 0; i < kValidNColorsCount; i++) {
        [_colorSegment setLabel:[NSString stringWithFormat:@"%u", kValidNColors[i]] forSegment:i];
        [_colorSegment setWidth:0 forSegment:i]; // Auto-size
    }
    [_colorSegment setSelectedSegment:[self indexForNColors:_config.ncolors]];
    [contentView addSubview:_colorSegment];

    y -= rowHeight + 10;

    // Anti-aliasing row
    NSTextField *msaaLabel = [[NSTextField alloc] initWithFrame:NSMakeRect(margin, y, labelWidth, 20)];
    [msaaLabel setStringValue:@"Anti-aliasing:"];
    [msaaLabel setBezeled:NO];
    [msaaLabel setDrawsBackground:NO];
    [msaaLabel setEditable:NO];
    [msaaLabel setSelectable:NO];
    [msaaLabel setAlignment:NSTextAlignmentRight];
    [contentView addSubview:msaaLabel];

    _msaaSegment = [[NSSegmentedControl alloc] initWithFrame:NSMakeRect(controlX, y, controlWidth, 24)];
    [_msaaSegment setSegmentCount:kValidMSAACount];
    [_msaaSegment setLabel:@"Off" forSegment:0];
    [_msaaSegment setLabel:@"2x" forSegment:1];
    [_msaaSegment setLabel:@"4x" forSegment:2];
    [_msaaSegment setLabel:@"8x" forSegment:3];
    [_msaaSegment setSelectedSegment:[self indexForMSAA:_config.msaa_samples]];
    [contentView addSubview:_msaaSegment];

    y -= rowHeight + 20;

    // Buttons row
    CGFloat buttonWidth = 80;
    CGFloat buttonSpacing = 10;
    CGFloat buttonsX = windowRect.size.width - margin - (buttonWidth * 2) - buttonSpacing;

    NSButton *cancelButton = [[NSButton alloc] initWithFrame:NSMakeRect(buttonsX, margin, buttonWidth, 28)];
    [cancelButton setTitle:@"Cancel"];
    [cancelButton setBezelStyle:NSBezelStyleRounded];
    [cancelButton setTarget:self];
    [cancelButton setAction:@selector(cancelConfig:)];
    [cancelButton setKeyEquivalent:@"\033"]; // Escape key
    [contentView addSubview:cancelButton];

    NSButton *okButton = [[NSButton alloc] initWithFrame:NSMakeRect(buttonsX + buttonWidth + buttonSpacing, margin, buttonWidth, 28)];
    [okButton setTitle:@"OK"];
    [okButton setBezelStyle:NSBezelStyleRounded];
    [okButton setTarget:self];
    [okButton setAction:@selector(saveConfig:)];
    [okButton setKeyEquivalent:@"\r"]; // Return key
    [contentView addSubview:okButton];

    return self.configSheet;
}

- (void)fiberSliderChanged:(id)sender {
    NSInteger value = [_fiberSlider integerValue];
    [_fiberLabel setStringValue:[NSString stringWithFormat:@"%ld", (long)value]];
}

- (void)saveConfig:(id)sender {
    ScreenSaverDefaults *defaults = [self defaults];

    // Save fiber count
    uint32_t fiberCount = (uint32_t)[_fiberSlider integerValue];
    [defaults setInteger:fiberCount forKey:kFiberlampFiberCount];

    // Save color palette
    NSInteger colorIndex = [_colorSegment selectedSegment];
    if (colorIndex >= 0 && colorIndex < kValidNColorsCount) {
        [defaults setInteger:kValidNColors[colorIndex] forKey:kFiberlampNColors];
    }

    // Save MSAA
    NSInteger msaaIndex = [_msaaSegment selectedSegment];
    if (msaaIndex >= 0 && msaaIndex < kValidMSAACount) {
        [defaults setInteger:kValidMSAA[msaaIndex] forKey:kFiberlampMSAA];
    }

    [defaults synchronize];

    NSLog(@"Fiberlamp: Saved config - fibers=%u, colors=%u, msaa=%u",
          fiberCount,
          (colorIndex >= 0 && colorIndex < kValidNColorsCount) ? kValidNColors[colorIndex] : 0,
          (msaaIndex >= 0 && msaaIndex < kValidMSAACount) ? kValidMSAA[msaaIndex] : 0);

    // Close the sheet
    [NSApp endSheet:self.configSheet];
}

- (void)cancelConfig:(id)sender {
    // Close without saving
    [NSApp endSheet:self.configSheet];
}

@end
