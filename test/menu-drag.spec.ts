//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { expect } from 'chai';

import { Menu } from '../src/menu-renderer/menu';

type FakeNode = {
  renderData: {
    parent: FakeNode | null;
    position: { x: number; y: number } | undefined;
    nodeDiv: {
      classList: {
        add: (name: string) => void;
        remove: (name: string) => void;
        has: (name: string) => boolean;
      };
      style: { transform: string };
    };
  };
};

function makeNode(
  parent: FakeNode | null,
  position?: { x: number; y: number },
  transform = ''
): FakeNode {
  const classes = new Set<string>();
  return {
    renderData: {
      parent,
      position,
      nodeDiv: {
        classList: {
          add: (name: string) => {
            classes.add(name);
          },
          remove: (name: string) => {
            classes.delete(name);
          },
          has: (name: string) => classes.has(name),
        },
        style: { transform },
      },
    },
  };
}

// Regression test: when dragging across the parent wedge of a submenu, the parent item
// (e.g. root) becomes draggedItem. Leaving that wedge must not clear the parent's screen
// transform/position, or the whole menu jumps to (0, 0).
describe('Menu drag cleanup', () => {
  const invokeDragItem = (
    state: {
      centerItem: FakeNode;
      clickedItem: FakeNode | null;
      draggedItem: FakeNode | null;
    },
    item: FakeNode | null
  ) => {
    const proto = Menu.prototype as unknown as {
      dragItem: (item?: FakeNode | null) => void;
      clickItem: (item?: FakeNode | null) => void;
    };
    const instance = Object.assign(Object.create(proto), state);
    proto.dragItem.call(instance, item);
    state.clickedItem = instance.clickedItem;
    state.draggedItem = instance.draggedItem;
  };

  it('keeps parent transform and position when leaving the parent wedge', () => {
    const root = makeNode(null, { x: 960, y: 540 }, 'translate(960px, 540px)');
    const submenu = makeNode(root, { x: -200, y: -200 }, 'translate(-200px, -200px)');
    const child = makeNode(submenu, { x: 120, y: 40 }, 'translate(120px, 40px)');
    const state = {
      centerItem: submenu,
      clickedItem: null as FakeNode | null,
      draggedItem: null as FakeNode | null,
    };

    invokeDragItem(state, root);
    expect(root.renderData.nodeDiv.classList.has('dragged')).to.equal(true);

    invokeDragItem(state, child);
    expect(root.renderData.nodeDiv.classList.has('dragged')).to.equal(false);
    expect(root.renderData.nodeDiv.style.transform).to.equal('translate(960px, 540px)');
    expect(root.renderData.position).to.deep.equal({ x: 960, y: 540 });
  });

  it('clears child transform and position when a dragged child stops being dragged', () => {
    const root = makeNode(null, { x: 960, y: 540 }, 'translate(960px, 540px)');
    const submenu = makeNode(root, { x: -200, y: -200 }, 'translate(-200px, -200px)');
    const child = makeNode(submenu, { x: 120, y: 40 }, 'translate(120px, 40px)');
    const state = {
      centerItem: submenu,
      clickedItem: null as FakeNode | null,
      draggedItem: null as FakeNode | null,
    };

    invokeDragItem(state, child);
    invokeDragItem(state, null);
    expect(child.renderData.nodeDiv.classList.has('dragged')).to.equal(false);
    expect(child.renderData.nodeDiv.style.transform).to.equal('');
    expect(child.renderData.position).to.equal(undefined);
    expect(submenu.renderData.nodeDiv.style.transform).to.equal(
      'translate(-200px, -200px)'
    );
  });
});
