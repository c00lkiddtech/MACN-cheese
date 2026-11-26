#!/usr/bin/env python3
"""Turn iPadn's iOS Info.plist into a Mac Catalyst one for MACN-cheese."""
import plistlib, sys
src, dst, name, bundle_id = sys.argv[1:5]
p = plistlib.load(open(src, 'rb'))
subs = {'$(DEVELOPMENT_LANGUAGE)': 'en', '$(EXECUTABLE_NAME)': name, '$(PRODUCT_BUNDLE_IDENTIFIER)': bundle_id,
        '$(PRODUCT_NAME)': name, '$(MARKETING_VERSION)': '0.2.0', '$(CURRENT_PROJECT_VERSION)': '2'}
def fix(v):
    if isinstance(v, str):
        for a, b in subs.items(): v = v.replace(a, b)
        # MACN-cheese rebrand: identifiers first, then any visible name.
        return v.replace('app.local.iPadn', bundle_id).replace('iPadn', name)
    if isinstance(v, list): return [fix(x) for x in v]
    if isinstance(v, dict): return {k: fix(x) for k, x in v.items()}
    return v
p = fix(p)
for k in ['LSRequiresIPhoneOS', 'UISupportedInterfaceOrientations', 'UISupportedInterfaceOrientations~ipad',
          'UIBackgroundModes', 'NSSupportsLiveActivities', 'NSSupportsLiveActivitiesFrequentUpdates',
          'CADisableMinimumFrameDurationOnPhone', 'CFBundleIcons', 'CFBundleIcons~ipad', 'UILaunchScreen',
          'BGTaskSchedulerPermittedIdentifiers', 'UIRequiredDeviceCapabilities']:
    p.pop(k, None)
p.update({'CFBundleDisplayName': name, 'CFBundleName': name, 'CFBundleExecutable': name, 'CFBundleIdentifier': bundle_id,
          'CFBundleIconFile': 'AppIcon', 'CFBundleSupportedPlatforms': ['MacOSX'], 'LSMinimumSystemVersion': '26.0',
          'UIDeviceFamily': [2, 6], 'LSApplicationCategoryType': 'public.app-category.utilities',
          'NSHumanReadableCopyright': 'GPL-2.0. MACN-cheese is a macOS port of iPadn by mengangs.',
          'UIApplicationSupportsIndirectInputEvents': True})
for t in p.get('CFBundleURLTypes', []):
    t['CFBundleURLName'] = bundle_id
plistlib.dump(p, open(dst, 'wb'))
