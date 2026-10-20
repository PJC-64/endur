<script lang="ts">
  export interface CustomDialogState {
    show: boolean;
    type: "alert" | "confirm" | "prompt";
    title: string;
    message: string;
    inputValue: string;
    placeholder: string;
    resolve: ((value: any) => void) | null;
  }

  let {
    customDialog = $bindable()
  }: {
    customDialog: CustomDialogState;
  } = $props();

  let dialogElement: HTMLDialogElement | null = $state(null);

  $effect(() => {
    if (customDialog.show && dialogElement) {
      if (!dialogElement.open) {
        dialogElement.showModal();
      }
    } else if (!customDialog.show && dialogElement) {
      if (dialogElement.open) {
        dialogElement.close();
      }
    }
  });
</script>

{#if customDialog.show}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <dialog
    bind:this={dialogElement}
    class="custom-modal-dialog"
    onclose={() => {
      if (customDialog.show) {
        if (customDialog.type === "confirm") {
          customDialog.resolve?.(false);
        } else if (customDialog.type === "prompt") {
          customDialog.resolve?.(null);
        } else {
          customDialog.resolve?.(void 0);
        }
      }
    }}
    onclick={(e) => {
      if (dialogElement && e.target === dialogElement) {
        const rect = dialogElement.getBoundingClientRect();
        const clickInside = (
          rect.top <= e.clientY &&
          e.clientY <= rect.top + rect.height &&
          rect.left <= e.clientX &&
          e.clientX <= rect.left + rect.width
        );
        if (!clickInside) {
          dialogElement.close();
        }
      }
    }}
  >
    <div class="modal-content">
      <div class="modal-header">
        <h3>{customDialog.title}</h3>
      </div>
      <div class="modal-body">
        <p style="white-space: pre-line;">{customDialog.message}</p>
        {#if customDialog.type === "prompt"}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            type="text"
            class="modal-input"
            bind:value={customDialog.inputValue}
            placeholder={customDialog.placeholder || "Enter input..."}
            onkeydown={(e) => {
              if (e.key === "Enter") {
                customDialog.resolve?.(customDialog.inputValue);
              }
            }}
            autofocus
          />
        {/if}
      </div>
      <div class="modal-footer">
        {#if customDialog.type === "alert"}
          <button class="modal-btn confirm" onclick={() => customDialog.resolve?.(void 0)}>OK</button>
        {:else if customDialog.type === "confirm"}
          <button class="modal-btn cancel" onclick={() => customDialog.resolve?.(false)}>Cancel</button>
          <button class="modal-btn confirm" onclick={() => customDialog.resolve?.(true)}>Confirm</button>
        {:else if customDialog.type === "prompt"}
          <button class="modal-btn cancel" onclick={() => customDialog.resolve?.(null)}>Cancel</button>
          <button class="modal-btn confirm" onclick={() => customDialog.resolve?.(customDialog.inputValue)}>Submit</button>
        {/if}
      </div>
    </div>
  </dialog>
{/if}

<style>
  dialog.custom-modal-dialog {
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(15, 23, 42, 0.82);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border-radius: 12px;
    padding: 0;
    color: #f1f5f9;
    max-width: 460px;
    width: 90%;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.6);
    outline: none;
    overflow: hidden;
    animation: modal-appear 0.22s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  dialog.custom-modal-dialog::backdrop {
    background: rgba(4, 6, 10, 0.65);
    backdrop-filter: blur(5px);
    -webkit-backdrop-filter: blur(5px);
  }

  @keyframes modal-appear {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-8px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .modal-content {
    display: flex;
    flex-direction: column;
    padding: 1.5rem;
  }

  .modal-header h3 {
    margin: 0 0 0.75rem 0;
    font-size: 1.15rem;
    font-weight: 600;
    letter-spacing: -0.010em;
    color: #f8fafc;
  }

  .modal-body p {
    margin: 0 0 1.25rem 0;
    font-size: 0.9rem;
    line-height: 1.5;
    color: #94a3b8;
  }

  .modal-input {
    width: 100%;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    background-color: rgba(11, 15, 25, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #f8fafc;
    font-size: 0.9rem;
    font-family: inherit;
    outline: none;
    box-sizing: border-box;
    transition: all 0.2s ease;
    margin-bottom: 0.5rem;
  }

  .modal-input:focus {
    border-color: #06b6d4;
    box-shadow: 0 0 0 2px rgba(6, 182, 212, 0.15);
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  .modal-btn {
    padding: 0.5rem 1.1rem;
    font-size: 0.85rem;
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s ease;
    font-family: inherit;
    border: none;
  }

  .modal-btn.cancel {
    background: rgba(255, 255, 255, 0.04);
    color: #94a3b8;
    border: 1px solid rgba(255, 255, 255, 0.04);
  }

  .modal-btn.cancel:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #cbd5e1;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .modal-btn.confirm {
    background: #06b6d4;
    color: #0f172a;
    font-weight: 600;
  }

  .modal-btn.confirm:hover {
    background: #22d3ee;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(6, 182, 212, 0.25);
  }

  .modal-btn:active {
    transform: translateY(0);
  }
</style>
