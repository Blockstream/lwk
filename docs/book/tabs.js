/* from https://github.com/breez/breez-sdk-liquid-docs */

(function () {
    'use strict';

    let selected_ = null;

    customElements.define('custom-tabs', class extends HTMLElement {

        constructor() {
            super(); // always call super() first in the ctor.

            // Create shadow DOM for the component.
            let shadowRoot = this.attachShadow({ mode: 'open' });
            shadowRoot.innerHTML = `
          <style>
            :host {
              display: inline-block;
              contain: content;
              border: 1px solid var(--quote-border);
              border-radius: 8px;
              width: 100%;
            }
            #tabs {
              border-bottom: 1px solid var(--quote-border);
              background-color: var(--sidebar-bg);
              overflow-x: auto;
            }
            #tabs slot {
              display: inline-flex; /* Safari bug. Treats <slot> as a parent */
            }
            #tabs ::slotted(*) {
              color: var(--sidebar-fg);
              padding: 16px 8px;
              margin: 0;
              text-align: center;
              text-overflow: ellipsis;
              white-space: nowrap;
              overflow: hidden;
              cursor: pointer;
              border-top-left-radius: 8px;
              border-top-right-radius: 3px;
              border: none; /* if the user users a <button> */
            }
            #tabs ::slotted([tabindex="0"]), #tabs ::slotted(*:hover) {
              color: var(--sidebar-active);
            }
            #panels ::slotted([aria-hidden="true"]) {
              display: none;
            }
            pre {
              margin: 0;
            }
          </style>
          <div id="tabs">
            <slot id="tabsSlot" name="title"></slot>
          </div>
          <div id="panels">
            <slot id="panelsSlot"></slot>
          </div>
        `;
        }

        get selected() {
            return selected_;
        }

        set selected(idx) {
            selected_ = idx;
            this._selectTab(idx);
            this.setAttribute('selected', idx);
        }

        connectedCallback() {
            this.setAttribute('role', 'tablist');

            const tabsSlot = this.shadowRoot.querySelector('#tabsSlot');
            const panelsSlot = this.shadowRoot.querySelector('#panelsSlot');

            this.tabs = tabsSlot.assignedNodes({ flatten: true });
            this.panels = panelsSlot.assignedNodes({ flatten: true }).filter(el => {
                return el.nodeType === Node.ELEMENT_NODE;
            });

            // Save refer to we can remove listeners later.
            this._boundOnTitleClick = this._onTitleClick.bind(this);

            tabsSlot.addEventListener('click', this._boundOnTitleClick);
            document.addEventListener('mdbook-category-changed', this._onSiblingCategoryChanged.bind(this));
            this.selected = this._findFirstSelectedTab() || this._findStoredSelectedTab() || 0;
        }

        disconnectedCallback() {
            const tabsSlot = this.shadowRoot.querySelector('#tabsSlot');
            tabsSlot.removeEventListener('click', this._boundOnTitleClick);
            document.removeEventListener('mdbook-category-changed', this._onSiblingCategoryChanged.bind(this));
        }

        _onTitleClick(e) {
            if (e.target.slot === 'title') {
                this.selected = this.tabs.indexOf(e.target);
                e.target.focus();
            }
        }

        _findFirstSelectedTab() {
            let selectedIdx;
            for (let [i, tab] of this.tabs.entries()) {
                tab.setAttribute('role', 'tab');
                if (tab.hasAttribute('selected')) {
                    selectedIdx = i;
                }
            }
            return selectedIdx;
        }

        _findStoredSelectedTab() {
            let selectedIdx;
            if (this.getAttribute("category")) {
                let selectedText;
                try { selectedText = localStorage.getItem('mdbook-tabs-' + this.getAttribute("category")); } catch (e) { }
                if (selectedText) {
                    for (let [i, tab] of this.tabs.entries()) {
                        if (tab.textContent === selectedText) {
                            selectedIdx = i;
                            break;
                        }
                    }
                }
            }
            return selectedIdx;
        }

        _selectTab(idx = null, propagate = true) {
            let category = this.getAttribute("category");
            for (let i = 0, tab; tab = this.tabs[i]; ++i) {
                let select = i === idx;
                tab.setAttribute('tabindex', select ? 0 : -1);
                tab.setAttribute('aria-selected', select);
                this.panels[i].setAttribute('aria-hidden', !select);
                if (select && category && tab.textContent) {
                    try { localStorage.setItem('mdbook-tabs-' + category, tab.textContent); } catch (e) { }
                }
            }

            if (propagate) {
              document.dispatchEvent(new CustomEvent(
                'mdbook-category-changed', 
                { detail: { category: category, idx: idx }}
              ));
            }
        }

        _onSiblingCategoryChanged(e) {
            let category = this.getAttribute("category")
            if (category === e.detail.category) {
              this._selectTab(e.detail.idx, false);
              this.setAttribute('selected', e.detail.idx);
            }
        }
    });

})();
