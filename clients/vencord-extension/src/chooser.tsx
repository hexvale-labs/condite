/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { Button } from "@components/Button";
import type { CloudUpload, RenderModalProps } from "@vencord/discord-types";
import { closeModal, Modal, openModal, React, useState } from "@webpack/common";

import { useTranslation } from "./i18n";
import { conditeMaxBytes } from "./limits";
import type { Provider } from "./routing";

const pending = new Set<() => void>();

function Chooser({
    uploads,
    failure,
    finish,
    ...props
}: RenderModalProps & {
    uploads: CloudUpload[];
    failure: boolean;
    finish(value: Provider[] | null): void;
}) {
    const { t, locale, dir } = useTranslation();
    const [choices, setChoices] = useState<Provider[]>(
        uploads.map(u =>
            u.item.file.size <= conditeMaxBytes ? "condite" : "discord"
        )
    );
    const single = uploads.length === 1;
    const eligible = uploads.some(u => u.item.file.size <= conditeMaxBytes);
    return (
        <Modal
            {...props}
            title={failure ? t.failed : single ? t.upload : t.chooseDestination}
            subtitle={failure ? t.chooseDestination : t.draftHint}
            actions={
                single
                    ? undefined
                    : [
                          {
                              text: failure ? t.uploadCondite : t.upload,
                              variant: "primary",
                              disabled: failure && !eligible,
                              onClick: () => finish(choices)
                          },
                          {
                              text: failure ? t.keepAttachments : t.cancel,
                              variant: "secondary",
                              onClick: () => finish(null)
                          }
                      ]
            }
        >
            <div className="condite-chooser" lang={locale} dir={dir}>
                {uploads.map((upload, index) => (
                    <div
                        className="condite-choice-row"
                        key={`${upload.id}-${index}`}
                    >
                        <div className="condite-file-name">
                            <strong>{upload.filename}</strong>
                            <small>
                                {upload.item.file.size.toLocaleString(locale)}{" "}
                                {t.bytes}
                            </small>
                        </div>
                        {!single && (
                            <label>
                                {t.chooseDestination}{" "}
                                <select
                                    aria-label={`${t.chooseDestination}: ${upload.filename}`}
                                    value={choices[index]}
                                    onChange={event =>
                                        setChoices(previous =>
                                            previous.map((choice, i) =>
                                                i === index
                                                    ? (event.target
                                                          .value as Provider)
                                                    : choice
                                            )
                                        )
                                    }
                                >
                                    <option
                                        value="condite"
                                        disabled={
                                            upload.item.file.size >
                                            conditeMaxBytes
                                        }
                                    >
                                        Condite
                                    </option>
                                    <option value="discord">
                                        {failure
                                            ? t.keepAttachments
                                            : "Discord"}
                                    </option>
                                </select>
                            </label>
                        )}
                    </div>
                ))}
                {uploads.some(u => u.item.file.size > conditeMaxBytes) && (
                    <p className="condite-muted">{t.fileLimit}</p>
                )}
                {single && (
                    <div className="condite-actions">
                        <Button
                            disabled={!eligible}
                            onClick={() => finish(["condite"])}
                        >
                            {t.uploadCondite}
                        </Button>
                        <Button
                            variant="secondary"
                            onClick={() => finish(failure ? null : ["discord"])}
                        >
                            {failure ? t.keepAttachments : t.uploadDiscord}
                        </Button>
                    </div>
                )}
            </div>
        </Modal>
    );
}

export function chooseProviders(
    uploads: CloudUpload[],
    failure = false
): Promise<Provider[] | null> {
    return new Promise(resolve => {
        let done = false;
        const finish = (value: Provider[] | null) => {
            if (done) return;
            done = true;
            pending.delete(cancel);
            resolve(value);
            closeModal(key);
        };
        const cancel = () => finish(null);
        pending.add(cancel);
        const key = openModal(
            props => (
                <Chooser
                    {...props}
                    uploads={uploads}
                    failure={failure}
                    finish={finish}
                />
            ),
            {
                onCloseCallback: () => {
                    if (!done) {
                        done = true;
                        pending.delete(cancel);
                        resolve(null);
                    }
                }
            }
        );
    });
}

export function closeChoosers() {
    for (const cancel of [...pending]) cancel();
}
