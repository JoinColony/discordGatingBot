import { BrowserProvider } from 'ethers';

const connectButton = document.querySelector('#button-connect') as HTMLButtonElement;
const disconnectButton = document.querySelector('#button-disconnect') as HTMLButtonElement;
const errorText = document.querySelector('#text-error') as HTMLParagraphElement;
const successText = document.querySelector('#text-success') as HTMLParagraphElement;

const pathSplit = window.location.pathname.split('/');
const username = pathSplit[2]
const sessionId = pathSplit[3]

if (connectButton) {
  if (username) {
    connectButton.innerText = `${connectButton.innerText} as ${username}`;
  }
  connectButton.style.visibility = 'visible';

  connectButton.addEventListener('click', async () => {
    connectButton.disabled = true;

    // If MetaMask is installed there will be an `ethereum` object on the `window`
    const provider = new BrowserProvider((window as any).ethereum);

    // This will try to connect the page to MetaMask
    await provider.send('eth_requestAccounts', []);

    const signer = await provider.getSigner();
    const address = await signer.getAddress();

    const signature = await signer.signMessage(`Please sign this message to connect your Discord username ${username} with your wallet address. Session ID: ${sessionId}`);

    const response = await fetch(window.location.href, {
      method: 'POST',
      headers: {
        'Accept': 'application/json',
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        signature,
        address,
      })
    });

    if (response.ok) {
      connectButton.style.visibility = 'hidden';
      successText.style.visibility = 'visible';
      errorText.style.visibility = 'hidden';
    } else {
      errorText.style.visibility = 'visible';
      connectButton.disabled = false;
    }
  });
} else if (disconnectButton) {
  if (username) {
    disconnectButton.innerText = `${disconnectButton.innerText} as ${username}`;
  }
  disconnectButton.style.visibility = 'visible';

  disconnectButton.addEventListener('click', async () => {
    disconnectButton.disabled = true;
    const response = await fetch(window.location.href, {
      method: 'POST'
    });
    if (response.ok) {
      disconnectButton.style.visibility = 'hidden';
      successText.style.visibility = 'visible';
      errorText.style.visibility = 'hidden';
    } else {
      errorText.style.visibility = 'visible';
      disconnectButton.disabled = false;
    }
  });
}
