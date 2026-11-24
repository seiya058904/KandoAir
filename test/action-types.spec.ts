//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { expect } from 'chai';

import { ACTION_TYPES, ActionTypeRegistry } from '../src/common/action-type-registry';
import { WORKFLOW_ACTION_SCHEMA_V2 } from '../src/common/settings-schemata/menu-settings-v2';

// KandoAir supports nine single-step native actions. The native host has the same
// whitelist (see host/src/config.rs).
describe('action types', () => {
  it('offers the nine KandoAir actions', () => {
    expect(ACTION_TYPES).to.deep.equal([
      'launch-app',
      'open-file',
      'open-uri',
      'simulate-hotkey',
      'execute-command',
      'set-clipboard',
      'execute-macro',
      'focus-window',
      'open-menu',
    ]);
  });

  it('registers metadata for each action type', () => {
    ActionTypeRegistry.init();
    const registered = Object.keys(ActionTypeRegistry.getInstance().getAllMetadata());
    expect(registered).to.deep.equal(ACTION_TYPES);
  });

  it('matches the menu settings schema', () => {
    const schemaTypes = WORKFLOW_ACTION_SCHEMA_V2.options.map(
      (option) => option.shape.type.value
    );
    expect([...schemaTypes].sort()).to.deep.equal([...ACTION_TYPES].sort());
  });

  it('rejects removed action types', () => {
    for (const type of ['delay', 'send-websocket-message', 'close-menu']) {
      expect(WORKFLOW_ACTION_SCHEMA_V2.safeParse({ type }).success).to.equal(false);
    }
  });
});
