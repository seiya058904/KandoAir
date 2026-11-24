//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { WindowWithAPIs } from '../../settings-window-api';
declare const window: WindowWithAPIs;

import React from 'react';

import * as classes from './WindowControls.module.scss';

/**
 * The minimize, maximize and close buttons of the settings window. Kando uses Electron's
 * title bar overlay for this; KandoAir draws them in the page because the settings window
 * has no system title bar.
 */
export default function WindowControls() {
  const [maximized, setMaximized] = React.useState(false);

  React.useEffect(() => {
    window.settingsAPI.isWindowMaximized().then(setMaximized);
    return window.settingsAPI.onMaximizedChanged(setMaximized);
  }, []);

  return (
    <div className={classes.windowControls}>
      <button
        aria-label="Minimize"
        className={classes.button}
        tabIndex={-1}
        type="button"
        onClick={() => window.settingsAPI.minimizeWindow()}>
        &#xE921;
      </button>
      <button
        aria-label={maximized ? 'Restore' : 'Maximize'}
        className={classes.button}
        tabIndex={-1}
        type="button"
        onClick={() => window.settingsAPI.toggleMaximizeWindow()}>
        {maximized ? '\uE923' : '\uE922'}
      </button>
      <button
        aria-label="Close"
        className={`${classes.button} ${classes.close}`}
        tabIndex={-1}
        type="button"
        onClick={() => window.settingsAPI.closeWindow()}>
        &#xE8BB;
      </button>
    </div>
  );
}
