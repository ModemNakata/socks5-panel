// Check if connection is HTTPS
if (window.location.protocol !== 'https:') {
    // alert('This app is only intended to be used with a secured connection. Redirecting to HTTPS...');
    window.location.href = 'https://' + window.location.host;
    // Prevent any further execution
    throw new Error('Redirecting to HTTPS');
}

fetch('/api/auth/register', {
    method: 'POST',
    credentials: 'include'
})
.then(response => {
    if (response.ok) {
        window.location.href = '/d';
    } else {
        document.getElementById('message').textContent = 'Something went wrong, please refresh the page.';
    }
})
.catch(() => {
    document.getElementById('message').textContent = 'Something went wrong, please refresh the page.';
});
