// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

import { observer } from 'mobx-react';
import React, { Component, PropTypes } from 'react';
import { FormattedMessage } from 'react-intl';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import { newError } from '~/redux/actions';
import shapeshiftBtn from '~/../assets/images/shapeshift-btn.png';
import HardwareStore from '~/mobx/hardwareStore';
import ExportStore from '~/modals/ExportAccount/exportStore';
import { DeleteAccount, EditMeta, Faucet, PasswordManager, Shapeshift, Transfer, Verification } from '~/modals';
import { setVisibleAccounts } from '~/redux/providers/personalActions';
import { fetchCertifiers, fetchCertifications } from '~/redux/providers/certifications/actions';
import { Actionbar, Button, ConfirmDialog, Input, Page, Portal } from '~/ui';
import { DeleteIcon, DialIcon, EditIcon, LockedIcon, SendIcon, VerifyIcon, FileDownloadIcon } from '~/ui/Icons';

import DeleteAddress from '../Address/Delete';

import Header from './Header';
import Store from './store';
import Transactions from './Transactions';
import styles from './account.css';

@observer
class Account extends Component {
  static contextTypes = {
    api: PropTypes.object.isRequired
  };

  static propTypes = {
    fetchCertifiers: PropTypes.func.isRequired,
    fetchCertifications: PropTypes.func.isRequired,
    setVisibleAccounts: PropTypes.func.isRequired,

    accounts: PropTypes.object,
    balances: PropTypes.object,
    certifications: PropTypes.object,
    netVersion: PropTypes.string.isRequired,
    newError: PropTypes.func,
    params: PropTypes.object
  }

  store = new Store();
  hwstore = HardwareStore.get(this.context.api);

  componentWillMount () {
    const { accounts, newError, params } = this.props;
    const { address } = params;

    this.exportStore = new ExportStore(this.context.api, accounts, newError, address);
  }

  componentDidMount () {
    this.props.fetchCertifiers();
    this.setVisibleAccounts();
  }

  componentWillReceiveProps (nextProps) {
    const prevAddress = this.props.params.address;
    const nextAddress = nextProps.params.address;

    if (prevAddress !== nextAddress) {
      this.setVisibleAccounts(nextProps);
    }
  }

  componentWillUnmount () {
    this.props.setVisibleAccounts([]);
  }

  setVisibleAccounts (props = this.props) {
    const { params, setVisibleAccounts, fetchCertifications } = props;
    const addresses = [params.address];

    setVisibleAccounts(addresses);
    fetchCertifications(params.address);
  }

  render () {
    const { accounts, balances } = this.props;
    const { address } = this.props.params;

    const account = (accounts || {})[address];
    const balance = (balances || {})[address];

    if (!account) {
      return null;
    }

    const isAvailable = !account.hardware || this.hwstore.isConnected(address);

    return (
      <div>
        { this.renderDeleteDialog(account) }
        { this.renderEditDialog(account) }
        { this.renderExportDialog() }
        { this.renderFaucetDialog() }
        { this.renderFundDialog() }
        { this.renderPasswordDialog(account) }
        { this.renderTransferDialog(account, balance) }
        { this.renderVerificationDialog() }
        { this.renderActionbar(account, balance) }
        <Page padded>
          <Header
            account={ account }
            balance={ balance }
            disabled={ !isAvailable }
          />
          <Transactions
            accounts={ accounts }
            address={ address }
          />
        </Page>
      </div>
    );
  }

  isKovan = (netVersion) => {
    return netVersion === '42';
  }

  isMainnet = (netVersion) => {
    return netVersion === '1';
  }

  isFaucettable = (netVersion, certifications, address) => {
    return this.isKovan(netVersion) || (
      this.isMainnet(netVersion) &&
      this.isSmsCertified(certifications, address)
    );
  }

  isSmsCertified = (_certifications, address) => {
    const certifications = _certifications && _certifications[address]
      ? _certifications[address].filter((cert) => cert.name.indexOf('smsverification') === 0)
      : [];

    return certifications.length !== 0;
  }

  renderActionbar (account, balance) {
    const { certifications, netVersion } = this.props;
    const { address } = this.props.params;
    const showTransferButton = !!(balance && balance.tokens);
    const isVerifiable = this.isMainnet(netVersion);
    const isFaucettable = this.isFaucettable(netVersion, certifications, address);

    const buttons = [
      <Button
        disabled={ !showTransferButton }
        icon={ <SendIcon /> }
        key='transferFunds'
        label={
          <FormattedMessage
            id='account.button.transfer'
            defaultMessage='transfer'
          />
        }
        onClick={ this.store.toggleTransferDialog }
      />,
      <Button
        icon={
          <img
            className={ styles.btnicon }
            src={ shapeshiftBtn }
          />
        }
        key='shapeshift'
        label={
          <FormattedMessage
            id='account.button.shapeshift'
            defaultMessage='shapeshift'
          />
        }
        onClick={ this.store.toggleFundDialog }
      />,
      isVerifiable
        ? (
          <Button
            icon={ <VerifyIcon /> }
            key='verification'
            label={
              <FormattedMessage
                id='account.button.verify'
                defaultMessage='verify'
              />
            }
            onClick={ this.store.toggleVerificationDialog }
          />
        )
        : null,
      isFaucettable
        ? (
          <Button
            icon={ <DialIcon /> }
            key='faucet'
            label={
              <FormattedMessage
                id='account.button.faucet'
                defaultMessage='Kovan ETH'
              />
            }
            onClick={ this.store.toggleFaucetDialog }
          />
        )
        : null,
      <Button
        icon={ <EditIcon /> }
        key='editmeta'
        label={
          <FormattedMessage
            id='account.button.edit'
            defaultMessage='edit'
          />
        }
        onClick={ this.store.toggleEditDialog }
      />,
      <Button
        icon={ <FileDownloadIcon /> }
        key='exportmeta'
        label={
          <FormattedMessage
            id='account.button.export'
            defaultMessage='export'
          />
        }
        onClick={ this.store.toggleExportDialog }
      />,
      !account.hardware && (
        <Button
          icon={ <LockedIcon /> }
          key='passwordManager'
          label={
            <FormattedMessage
              id='account.button.password'
              defaultMessage='password'
            />
          }
          onClick={ this.store.togglePasswordDialog }
        />
      ),
      <Button
        icon={ <DeleteIcon /> }
        key='delete'
        label={
          <FormattedMessage
            id='account.button.delete'
            defaultMessage='delete'
          />
        }
        onClick={ this.store.toggleDeleteDialog }
      />
    ];

    return (
      <Actionbar
        buttons={ buttons }
        title={
          <FormattedMessage
            id='account.title'
            defaultMessage='Account Management'
          />
        }
      />
    );
  }

  renderDeleteDialog (account) {
    if (!this.store.isDeleteVisible) {
      return null;
    }

    if (account.hardware) {
      return (
        <DeleteAddress
          account={ account }
          confirmMessage={
            <FormattedMessage
              id='account.hardware.confirmDelete'
              defaultMessage='Are you sure you want to remove the following hardware address from your account list?'
            />
          }
          visible
          route='/accounts'
          onClose={ this.store.toggleDeleteDialog }
        />
      );
    }

    return (
      <DeleteAccount
        account={ account }
        onClose={ this.store.toggleDeleteDialog }
      />
    );
  }

  renderEditDialog (account) {
    if (!this.store.isEditVisible) {
      return null;
    }

    return (
      <EditMeta
        account={ account }
        onClose={ this.store.toggleEditDialog }
      />
    );
  }

  renderExportDialog () {
    const { toggleExportDialog } = this.store;
    const { changePassword, accountValue, onExport } = this.exportStore;

    if (!this.store.isExportVisible) {
      return null;
    }
    return (
      <Portal
        open
        isSmallModal
        onClose={ toggleExportDialog }
      >
        <ConfirmDialog
          open
          disabledConfirm={ false }
          labelConfirm='Export'
          labelDeny='Cancel'
          onConfirm={ onExport }
          onDeny={ toggleExportDialog }
          title={
            <FormattedMessage
              id='export.account.title'
              defaultMessage='Export Account'
            />
          }
        >
          <div className={ styles.textbox }>
            <FormattedMessage
              id='export.account.info'
              defaultMessage='Export your account as a JSON file. Please enter the password linked with this account.'
            />
          </div>
          <Input
            className={ styles.textbox }
            autoFocus
            type='password'
            hint={
              <FormattedMessage
                id='export.account.password.hint'
                defaultMessage='The password specified when creating this account'
              />
            }
            label={
              <FormattedMessage
                id='export.account.password.label'
                defaultMessage='Account password'
              />
            }
            onChange={ changePassword }
            value={ accountValue }
          />
        </ConfirmDialog>
      </Portal>
    );
  }

  renderFaucetDialog () {
    const { netVersion } = this.props;

    if (!this.store.isFaucetVisible) {
      return null;
    }

    const { address } = this.props.params;

    return (
      <Faucet
        address={ address }
        netVersion={ netVersion }
        onClose={ this.store.toggleFaucetDialog }
      />
    );
  }

  renderFundDialog () {
    if (!this.store.isFundVisible) {
      return null;
    }

    const { address } = this.props.params;

    return (
      <Shapeshift
        address={ address }
        onClose={ this.store.toggleFundDialog }
      />
    );
  }

  renderPasswordDialog (account) {
    if (!this.store.isPasswordVisible) {
      return null;
    }

    return (
      <PasswordManager
        account={ account }
        onClose={ this.store.togglePasswordDialog }
      />
    );
  }

  renderTransferDialog (account, balance) {
    if (!this.store.isTransferVisible) {
      return null;
    }

    const { balances } = this.props;

    return (
      <Transfer
        account={ account }
        balance={ balance }
        balances={ balances }
        onClose={ this.store.toggleTransferDialog }
      />
    );
  }

  renderVerificationDialog () {
    if (!this.store.isVerificationVisible) {
      return null;
    }

    const { address } = this.props.params;

    return (
      <Verification
        account={ address }
        onClose={ this.store.toggleVerificationDialog }
      />
    );
  }
}

function mapStateToProps (state) {
  const { accounts } = state.personal;
  const { balances } = state.balances;
  const certifications = state.certifications;
  const { netVersion } = state.nodeStatus;

  return {
    accounts,
    balances,
    certifications,
    netVersion
  };
}

function mapDispatchToProps (dispatch) {
  return bindActionCreators({
    fetchCertifiers,
    fetchCertifications,
    newError,
    setVisibleAccounts
  }, dispatch);
}

export default connect(
  mapStateToProps,
  mapDispatchToProps
)(Account);
