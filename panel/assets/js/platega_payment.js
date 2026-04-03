const PLATEGA_MODAL_HTML = `
<div class="modal-overlay" id="platega-modal">
    <div class="modal">
        <h3>Platega Payment</h3>
        <p class="platega-description">Enter the amount you want to deposit in Russian Rubles.</p>
        
        <form id="platega-payment-form">
            <div class="form-group">
                <label for="platega-amount">Amount (RUB)</label>
                <input type="number" id="platega-amount" min="1" step="1" placeholder="Enter amount" required>
            </div>

            <div class="payment-method-selector">
                <p class="payment-method-label">Payment Method</p>
                <div class="payment-method-options">
                    <label class="payment-method-option">
                        <input type="radio" name="payment-method" value="spb" id="spb-option" checked>
                        <div class="method-content">
                            <div class="method-icon">
                                <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <rect x="3" y="3" width="7" height="7"/>
                                    <rect x="14" y="3" width="7" height="7"/>
                                    <rect x="3" y="14" width="7" height="7"/>
                                    <rect x="14" y="14" width="7" height="7"/>
                                </svg>
                            </div>
                            <div class="method-info">
                                <span class="method-name">SBP</span>
                                <span class="method-desc">System of Fast Payments - QR code payment</span>
                            </div>
                        </div>
                    </label>
                    <label class="payment-method-option">
                        <input type="radio" name="payment-method" value="card">
                        <div class="method-content">
                            <div class="method-icon">
                                <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <rect x="1" y="4" width="22" height="16" rx="2" ry="2"/>
                                    <line x1="1" y1="10" x2="23" y2="10"/>
                                </svg>
                            </div>
                            <div class="method-info">
                                <span class="method-name">Card</span>
                                <span class="method-desc">Russian bank card</span>
                            </div>
                        </div>
                    </label>
                </div>
            </div>

            <div id="platega-error" class="error-message"></div>

            <button type="submit" class="btn-big btn-primary" id="platega-submit" style="width: 100%; margin-top: 1rem; margin-bottom: 1rem;">
                Continue to Payment
            </button>
        </form>

        <button class="modal-close" id="close-platega-modal" style="background: var(--bg-card-hover); color: var(--text); margin-top: 0.75rem;">Cancel</button>
    </div>
</div>
`;

document.addEventListener('DOMContentLoaded', () => {
    document.body.insertAdjacentHTML('beforeend', PLATEGA_MODAL_HTML);

    const plategaModal = document.getElementById('platega-modal');
    const closePlategaModalBtn = document.getElementById('close-platega-modal');
    const plategaForm = document.getElementById('platega-payment-form');
    const amountInput = document.getElementById('platega-amount');
    const errorMessage = document.getElementById('platega-error');
    const submitBtn = document.getElementById('platega-submit');

    if (!plategaModal) return;

    function openPlategaModal() {
        plategaModal.classList.add('active');
        errorMessage.textContent = '';
        amountInput.value = '';
        document.getElementById('spb-option').checked = true;
    }

    function closePlategaModal() {
        plategaModal.classList.remove('active');
    }

    if (closePlategaModalBtn) {
        closePlategaModalBtn.addEventListener('click', closePlategaModal);
    }

    plategaModal.addEventListener('click', (e) => {
        if (e.target.id === 'platega-modal') {
            closePlategaModal();
        }
    });

    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && plategaModal.classList.contains('active')) {
            closePlategaModal();
        }
    });

    if (plategaForm) {
        plategaForm.addEventListener('submit', async (e) => {
            e.preventDefault();
            
            const amount = parseFloat(amountInput.value);
            if (isNaN(amount) || amount < 1) {
                errorMessage.textContent = 'Minimum amount is 1 RUB';
                return;
            }

            const method = document.querySelector('input[name="payment-method"]:checked').value;
            
            submitBtn.disabled = true;
            submitBtn.textContent = 'Opening payment page...';
            errorMessage.textContent = '';

            try {
                window.open(`/payment/process?provider=platega&method=${method}&amount=${amount}`, '_blank');
                closePlategaModal();
            } catch (err) {
                errorMessage.textContent = 'Failed to open payment page';
            } finally {
                submitBtn.disabled = false;
                submitBtn.textContent = 'Continue to Payment';
            }
        });
    }

    window.openPlategaPaymentModal = openPlategaModal;
});
