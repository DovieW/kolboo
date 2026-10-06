import {
    useRequestLicenseEmailCode, useVerifyLicenseEmailCode,
    useStartLicenseLogin, useRequestLicensePasswordReset,
} from "../../lib/queries";
import { licenseAPI } from "../../lib/tauri";
import { EmailCodeSignIn } from "./EmailCodeSignIn";
import { isCloudServiceAvailable } from "../../lib/cloudService";

export function AccountAuthentication() {
    const request = useRequestLicenseEmailCode();
    const verify = useVerifyLicenseEmailCode();
    const login = useStartLicenseLogin();
    const reset = useRequestLicensePasswordReset();
    if (!isCloudServiceAvailable()) return <p>Accounts will be available in a later update. Use your own provider keys in Settings.</p>;
    return <EmailCodeSignIn actions={{
        requestCode: email => request.mutateAsync(email),
        verifyCode: (email, code) => verify.mutateAsync({ email, code }),
        passwordSignIn: (email, password) => login.mutateAsync({ email, password }),
        browserSignIn: () => login.mutateAsync(undefined),
        resetPassword: email => reset.mutateAsync(email),
        cancel: () => licenseAPI.cancelLogin(),
    }} />;
}
